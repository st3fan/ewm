# Debugging Tools — WozBug and Friends

A working document for building debugging support *into* EWM, so the next
bug hunt starts from tools instead of temporary code. In the house style
of `JSON_CONFIG.md` / `MAC_APP.md`: re-read at the start of every session,
update as phases land. **Every phase passes the full gates** (`cargo fmt
--check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`).

## Status

| Phase | Description | Size | Status |
|---|---|---|---|
| 1 | CPU breakpoints + the WozBug command core (library, used by tests) | S/M | **Done** |
| 2 | The `--wozbug` line server + `--break` flag + device commands | M | **Done** |
| 3 | Watchpoints, runtime trace toggle, disassembly, symbol table | M | Not started |

## Phase 1 decisions (recorded as built)

- **Breakpoint overhead is zero, as theorized**: the Dormann suite
  (release) runs in 0.19s both before and after the `step()` breakpoint
  check — the empty-`Vec` branch is unmeasurable, so breakpoints live in
  the normal build unconditionally. No debug-build split exists.
- **Stopped semantics**: a hit (or `Cpu::stop()`) makes `step()` return 0
  without executing; `resume()` clears it and skips the breakpoint at the
  current PC once, so resume-then-step makes progress. **Burst loops must
  check `stopped()`** or they spin on the 0-cycle steps.
- **`wozbug::WozBug::execute(&mut Two, &str) -> String`** is the whole
  core; the only session state is the dot address. `G` clears the stopped
  state and leaves running to the caller.
- **Bus-honest dumps**: memory reads go through the bus, soft-switch side
  effects included — authentic, documented in `?` help. A side-effect-free
  peek is a Phase 3+ question if it ever bites.
- The #253 retrofit is `ewm/tests/wozbug.rs`: boot DOS, `B RWTS` (by
  symbol), CATALOG lands on the breakpoint, `R` names `PC=BD00 (RWTS)`,
  `DSK` shows the controller, Y/A → IOB checked, `S`/`G` resume cleanly.

## Phase 2 decisions (recorded as built)

- **`--wozbug [port]`** (default 6502) starts the line server on
  127.0.0.1; **`--break addr[,addr]`** (hex or symbols — `--break RWTS`)
  arms breakpoints at boot and implies the server. Optional-value parsing
  follows the `--color` peek convention.
- **Threading**: the server threads only move strings over channels — the
  frame loop drains commands and runs `execute()` on the machine's own
  thread, so no locking exists and an idle server costs one `try_recv`
  per loop iteration. One client at a time; the writer uses a 200ms
  `recv_timeout` so a silent disconnect frees it to accept the next
  connection.
- **Stop integration**: the frame burst breaks on a 0-cycle step; a
  rising stopped edge sends `stopped at …` + registers to the client
  *and* stderr (so a hit is visible even before a client connects — the
  announcement a later client misses is retold by `R`'s `[stopped]`).
- **No stdin REPL** (open question resolved): the server covers scripted
  and interactive use via `nc`; the tty adds a raw-mode/SDL headache for
  no new capability.
- Verified live: `ewm two --break RWTS --set machine:slots:6:drive1=…`,
  then `nc localhost 6502` → announcement on connect, `R` shows the IOB
  pointer in Y/A, `S 3` traces `STY $48 / STA $49 / LDY #$02`, `G`
  resumes into the boot.

## The motivating case (what PR #253 actually took)

Debugging the multi-controller CATALOG hang meant writing a **throwaway
example binary** that booted the machine, typed the command through the
keyboard latch, and every 5M cycles printed the PC plus hand-picked device
state (`half_track`, `active_drive`, `drive_lit`). That worked, but every
step of it should have been a tool:

- *"Where is it stuck?"* → sampling `two.cpu.pc` by hand. Wanted: a
  breakpoint at `$BD25`, or just asking a live machine for its registers.
- *"What is the controller doing?"* → ad-hoc `eprintln!` of `Dsk`
  accessors. Wanted: a `DSK` command that dumps every controller's state.
- *"What code is `$BD25`?"* → recalling from memory that `$BD00` is the
  DOS 3.3 RWTS entry. Wanted: a small symbol table that prints
  `BD25 (RWTS+$25)`.
- *"What's on screen?"* → `text_screen()` calls sprinkled around. Wanted:
  a `TEXT` command.

None of this needs a big debugger. It needs a **small, always-available
one** — which is exactly the lesson of Apple's MicroBug.

## Prior art: MicroBug and MacsBug (TN1136)

Apple shipped **MicroBug** in every Mac ROM from the Plus onward: the
debugger you got when the NMI fired and MacsBug wasn't installed. Its
entire command set was `DM` (dump memory), `SM` (set memory), `G [addr]`,
`TD` (all registers), and `Ax/Dx/PC/SR` (display/set one register), over a
tiny expression language (hex, `.` = the "dot address" remembered from the
last dump, `@expr` indirection, `+`/`-`). Lessons worth stealing:

- **Tiny is sufficient.** Dump, set, registers, go, breakpoints — that
  covers the #253 session and most sessions like it.
- **The dot address.** `DM` remembers where it was; a bare Return
  continues the dump. Browsing memory becomes two keystrokes.
- **Always present beats powerful.** MicroBug existed for the bugs that
  *disappear when you install MacsBug*. Our analogue: features that live
  in the normal build (when they're free) are worth more than a fancy
  debug-only build nobody runs.
- From **MacsBug**, one idea matters more than all its power features:
  domain-aware commands (`dcmds`, templates, heap checks). A debugger that
  knows the *machine* — controllers, soft switches, the text screen — is
  worth ten generic memory dumpers. `DSK` is our `hz` (heap zone).

And EWM has something the Mac never did: the machine's own debugger
heritage. The Apple II Monitor (`CALL -151`) syntax — `280.29F` to dump,
`300:A9 20` to deposit, `300G` to go, `L` to disassemble — is the natural
dialect for this emulator's debugger. Hence:

## WozBug

A minimal monitor, MicroBug in role, Woz Monitor in dialect. One command
core with three frontends:

```
                    ┌──────────────────────────────┐
   tests ──────────▶│  wozbug::execute(             │
                    │      &mut Two, &str) -> String│◀───── line server
   (direct calls)   └──────────────────────────────┘        (nc/telnet)
                                   ▲
                                   │ on breakpoint / --break
                             the SDL loop pauses
```

- **Library first.** `wozbug::execute(&mut Two, cmd) -> String` is a pure
  function over the machine. Integration tests call it directly — the
  #253 test's hand-rolled state assertions become
  `assert!(wozbug(&mut two, "DSK").contains("S6 D1 track 17"))`-style
  checks, and the interactive frontends are thin wrappers.
- **Line-oriented TCP server, not HTTP.** `ewm two --wozbug [port]`
  (default: 6502, of course) accepts one connection and speaks
  newline-delimited commands — trivially driven by a human with `nc`, or
  by Claude with `printf 'R\nDSK\n' | nc -w1 localhost 6502`. HTTP+JSON
  adds nothing for this use and costs a dependency; if tooling ever wants
  structure, a JSON output mode (`R -j`) or an HTTP façade can be layered
  on the same core later.
- **Pause semantics.** The server thread only queues lines; the SDL loop
  drains and executes them between frames (same thread as the machine, no
  locking). Commands work against the *running* machine; a breakpoint or
  an explicit `STOP` command pauses emulation (the existing Cmd-P pause
  path) until `G`.

### Command sketch (Woz Monitor dialect where it fits)

| Command | Does | Heritage |
|---|---|---|
| `280.29F` | hex+ASCII dump of a range | Woz Monitor |
| `280` | examine one byte; sets the dot address | Woz Monitor |
| *(Return)* | continue the last dump from the dot address | MicroBug `DM` |
| `300:A9 8D 20` | deposit bytes | Woz Monitor |
| `R` | registers: `A X Y SP PC P` + decoded flags + symbol for PC | MicroBug `TD` |
| `A=FF` `PC=BD00` | set a register | MicroBug `Ax` |
| `G` / `300G` | resume / go from address | both |
| `S` / `S 20` | step 1 / n instructions, printing trace lines | MacsBug `t` |
| `B BD25` / `B` / `B-BD25` | set / list / clear breakpoints | — |
| `DSK` | every controller: slot, selected drive, half-track, motor, media | dcmd lesson |
| `SW` | soft-switch state (text/mixed/page2/hires, 80col, banks…) | dcmd lesson |
| `TEXT` | the rendered text screen | dcmd lesson |
| `SLOTS` | the machine's slot table | dcmd lesson |
| `T` / `T-` | toggle the CPU trace (the `--trace` machinery) at runtime | — |

Parsing stays MicroBug-forgiving: hex everywhere, no `$`/`0x` required,
unknown input answers with a beep… or at least a `?` (the Monitor's own
error message).

### Breakpoints in the core: the cheap-theory is probably right

The owner's theory — "maybe breakpoints are simple stuff with no
overhead" — is very likely correct and should be validated first thing in
Phase 1:

- `Cpu` gains `breakpoints: Vec<u16>` and `step()` checks
  `!breakpoints.is_empty() && breakpoints.contains(&pc)` before executing.
  With no breakpoints set that is one always-false, perfectly-predicted
  branch per instruction. Expected cost: unmeasurable. **Measure it**
  against the WOZ sweep (`zz_woz_sweep` is our de-facto CPU benchmark)
  before assuming; if it truly vanishes, breakpoints live in the normal
  build unconditionally and no debug-build split exists at all.
- A hit sets a `stopped` reason instead of executing; the frontends decide
  what pausing means (tests: return from `step_until`-style loops; SDL
  loop: enter pause + announce on the wozbug connection / stderr).
- **Watchpoints** (break on read/write of an address or range) sit on the
  memory bus — every access, hotter than the PC check. Same empty-check
  pattern, but this one might show up; measure, and if it costs, gate it
  behind a `debugger` cargo feature *(and accept that watchpoints then
  need a debug build — fine; breakpoints are the always-present baseline)*.
- Verbose memory-access logging (the "chatty debug build" idea) becomes
  redundant if watchpoints exist: `W C0C0.C0CF` logging accesses to one
  DEVSEL range is strictly better than a firehose of every bus access.

### CLI instrumentation

- `--break BD25[,C600]` — arm breakpoints at boot and start paused-on-hit;
  implies `--wozbug` so there is somewhere to land. This alone would have
  turned the #253 session into a two-minute look.
- `--wozbug [port]` — the server, machine running normally.
- Later, if wanted: `--log dsk,iou` scoped runtime logging as the grown-up
  replacement for the current all-or-nothing `--debug`.

### A small symbol table

A built-in, read-only map of the addresses this machine's software lives
at: Monitor entry points ($FF69, $FDED COUT, $FD0C RDKEY…), the DOS 3.3
RWTS/file-manager landmarks ($3D0, $9D00, $B7E8 IOB, $BD00 RWTS), ProDOS
MLI ($BF00). `R` and `S` print `PC=BD25 (RWTS+$25)`; `B RWTS` works as an
address. Tiny (a static array), high leverage — in #253 this would have
named the hang site instantly.

## Phases

**Phase 1 (S/M)** — the core, test-facing: `breakpoints` in `Cpu` (+ the
overhead measurement recorded here), the `wozbug` module with `execute()`
covering `R`, dump/deposit/dot-address, `G`/`S`/`B`, `DSK`/`SW`/`TEXT`/
`SLOTS`, the symbol table; integration tests use it (retrofit one #253
assertion as proof).

**Phase 2 (M)** — the interactive surface: `--wozbug` line server wired
into the SDL loop (queue + between-frames execution, pause/resume),
`--break`, breakpoint-hit announcement, README section.

**Phase 3 (M)** — the power-ups, each optional and demand-driven:
watchpoints (with the feature-gate decision), runtime trace toggle `T`,
`L` disassembly (`ewm-core/fmt.rs` already disassembles for `--trace` —
reuse it), JSON output mode if tooling wants it.

## Open questions

- **Interactive entry without the server**: is stdin a useful REPL when
  EWM is launched from a terminal (breakpoint hit → prompt on the
  controlling tty)? Nice for humans; the server already covers Claude.
  Decide in Phase 2.
- **Step-over / run-to-RTS**: `S` is per-instruction; RWTS-sized hunts
  want "run until return". Cheap to add on top of breakpoints (break at
  the return address from the stack) — Phase 3 candidate.
- **One connection or many**: one is enough; rejecting a second keeps the
  server trivial.
- **Where symbols stop**: monitor/DOS/ProDOS landmarks yes; per-program
  symbols no — this is MicroBug, not MacsBug.
