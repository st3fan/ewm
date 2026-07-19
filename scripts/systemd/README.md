# Serving an Apple 1 on Port 6502

`telnet your-host 6502` → a freshly powered-on Apple 1 at the Woz
Monitor prompt. One machine per connection, gone on hangup.

How it works: systemd owns the socket (inetd-style `Accept=yes`) and
starts one `ewm-one@.service` instance per connection with the TCP
stream as stdin/stdout (`StandardInput=socket`). The emulator does no
networking — `ewm one --tty` just speaks bytes: keyboard in, display
out, at a wall-clock-true 1.023 MHz. Design and as-built notes:
`notes/APPLE1.md`; the plan was `plans/20260719-04-apple1-telnet.md`.

## Requirements

- A Linux server with systemd.
- A Rust toolchain and **CMake** (the build compiles SDL3 from source).
- **SDL3's build dependencies** — install the list from
  <https://wiki.libsdl.org/SDL3/README-linux#build-dependencies>
  (on Debian/Ubuntu that page has a single `apt-get install …` line to
  paste). The headless build needs far less of SDL than a desktop
  build, but SDL's CMake configure still probes the system, and having
  the full set installed is the reliable path — a partial set is how
  you meet `Couldn't find dependency package for XTEST` (see
  Troubleshooting).

## 1. Build

From a repo checkout on the server:

```sh
cargo build --release -p ewm --no-default-features --features sdl-static-headless
```

`sdl-static-headless` statically links an SDL3 **Unix console build**:
the binary is self-contained (no SDL libraries needed at runtime — the
machine configs and ROM images are embedded too). Only `--tty` runs on
such a binary; the SDL window frontends need a normal build.

## 2. Install

```sh
sudo install -m 755 target/release/ewm /usr/local/bin/ewm
sudo install -D -m 644 scripts/systemd/banner.txt /usr/local/share/ewm/banner.txt
sudo cp scripts/systemd/ewm-one.socket scripts/systemd/ewm-one@.service /etc/systemd/system/
sudo systemctl daemon-reload
```

The paths matter: the service unit runs `/usr/local/bin/ewm` with
`--tty-banner /usr/local/share/ewm/banner.txt`. Edit `banner.txt` to
taste — it is what greets every caller before the machine boots.

## 3. Enable

```sh
sudo systemctl enable --now ewm-one.socket
systemctl status ewm-one.socket        # should say: Listening on [::]:6502
```

## 4. Try it

```sh
telnet localhost 6502
```

You get the banner, then the monitor's `\` prompt. Things to try are
in the banner: `FF00.FF1F` dumps ROM, `E000R` starts Integer BASIC.
**Meta-R** (or telnet's `send brk` after `Ctrl-]`) is the RESET
button; `Ctrl-]` then `quit` leaves telnet. Each line appears once —
the served unit runs `--tty-telnet`, which tells your telnet client to
stop echoing locally so only the machine's own echo shows. `nc
localhost 6502` works too (it just sees a few telnet negotiation bytes
at the very start, which it renders as harmless garbage).

Each active session is its own unit instance:

```sh
systemctl list-units 'ewm-one@*'       # who's on the machine room floor
journalctl -u 'ewm-one@*'              # per-instance stderr (errors land here)
```

## Operating it

- **A different machine**: edit `ExecStart` in `ewm-one@.service` —
  `--config builtin:replica1` serves the Replica 1 (Krusader lives at
  `F000R`). Any config source works, but a config *file* needs a path
  readable under the sandbox (`/usr/local/share/ewm/` is).
- **A different port**: `ListenStream=` in `ewm-one.socket`.
- **Limits**, all in the units: `MaxConnectionsPerSource=4` (per-IP),
  `RuntimeMaxSec=4h` (abandoned sessions do not keep a 1 MHz loop
  forever), `MemoryMax=64M`.
- **Local testing without systemd**: `ewm one --tty` in any terminal,
  or `nc -l 6502 --sh-exec 'ewm one --tty-telnet'`. (Bare `--tty` is
  byte-clean — no telnet negotiation — for a local terminal or a raw
  pipe; `--tty-telnet` adds the negotiation that suppresses a telnet
  client's local echo, and is what the systemd unit uses.)

## Security posture

This is a toy, deliberately hardened like it isn't: each session runs
as an ephemeral `DynamicUser` under `ProtectSystem=strict` /
`ProtectHome=yes` / `NoNewPrivileges` — the process can read the
banner and write to the journal, and that is about it. Telnet is
plaintext; there is nothing secret on an Apple 1, but don't put this
on the open internet expecting it to stay quiet — the per-source and
runtime limits are the polite fence, not a security boundary.

## Troubleshooting

- **`Couldn't find dependency package for XTEST` (CMake)** — SDL found
  part of the X11 dev stack but not all of it. Install the full build
  dependency list from the SDL wiki page above, then clear SDL's CMake
  cache before rebuilding — a failed configure leaves state behind:

  ```sh
  cargo clean -p sdl3-sys
  cargo build --release -p ewm --no-default-features --features sdl-static-headless
  ```

- **`Unit ewm-one.socket does not exist`** — the units live in this
  repo directory until you copy them to `/etc/systemd/system/` and run
  `systemctl daemon-reload` (step 2).
- **Connection opens, then closes immediately** — check
  `journalctl -u 'ewm-one@*'`; a missing banner file
  (`cannot read banner …`) or a bad `ExecStart` path lands there.
- **Every line appears twice** — your client is echoing locally on top
  of the machine's own echo. The served unit uses `--tty-telnet`, which
  suppresses that; if you see doubling, you're likely connected to a
  plain `--tty` instance (e.g. a local `ewm one --tty`, whose terminal
  echoes in cooked mode) or a telnet client that ignores the
  negotiation. For the systemd endpoint, confirm `ExecStart` says
  `--tty-telnet`.
- **A few junk characters at the very start of an `nc` session** —
  those are the telnet `WILL ECHO`/`WILL SGA` bytes (`--tty-telnet`
  announces them on connect); `nc` doesn't speak telnet, so it shows
  them raw. Harmless. Use a plain `--tty` instance if you want a
  byte-clean pipe.
