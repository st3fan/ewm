# The Apple 1 Family — Memory Maps, ROMs, and Machine Components

A working document for making the one-family machines fully
component-described: CPU, RAM banks, and ROM images specified in the
config document, with the ROM images embedded in the binary and
selectable as `builtin:<name>`. Plan:
`plans/20260719-03-one-machine-components.md`.

Reference: *Apple I Replica Creation* chapter 7
(https://www.applefritter.com/replica/chapter7).

## The real memory maps

**Apple 1 (1976):**

| Range | What |
|---|---|
| `$0000-$0FFF` | 4KB system/user RAM (expandable on board) |
| `$D010-$D013` | 6820 PIA — keyboard in, display out |
| `$E000-$EFFF` | 4KB RAM bank for Integer BASIC |
| `$FF00-$FFFF` | 256B Woz Monitor ROM (holds the reset vector) |

BASIC was **not in ROM** on the real machine: every power-on it was
loaded from cassette (or typed in) into the `$E000` RAM bank. CPU:
6502.

**Replica 1 (Briel Computers):**

| Range | What |
|---|---|
| `$0000-$7FFF` | 32KB RAM |
| `$D010-$D013` | 6821 PIA — same fixed I/O |
| `$E000-$EFFF` | Integer BASIC, now in ROM |
| `$F000-$FFFF` | ROM: Krusader assembler + monitor |

CPU: the boards ship with a 65C02 (the book's text says "6502"
generically; Krusader 1.3 exists in a 6502 and a 65C02 build for
exactly this reason).

## Do Krusader and Apple 1 BASIC overlap? (No.)

**They coexist, side by side, in the Replica 1's single 8KB ROM.**
Verified byte-by-byte against the images in this repo:

- `Krusader-1.3-{6502,65C02}` distributions are 8KB images mapped
  `$E000-$FFFF`, laid out as:
  - `$E000-$EFFF` — Woz's Integer BASIC (4KB; **identical** in both
    builds, and identical to `apple1-basic.rom`);
  - `$F000-$FEFF` — Krusader 1.3 itself (the builds diverge from
    `$F00D` on — the 65C02 build uses 65C02 opcodes);
  - `$FF00-$FFFF` — a **lightly modified** Woz Monitor (per build —
    the two builds' monitor pages differ from each other and from the
    pristine WozMon, which diverges at offset `$0F`).
- The repo's historical `rom/krusader.rom` **is byte-identical to the
  Krusader 1.3 6502 build**; `rom/apple1.rom` (256B) is the pristine
  WozMon.
- Finding from the comparison: today's emulated Replica 1 pairs a
  **65C02 CPU with the 6502 Krusader build** — harmless (6502 code
  runs on a 65C02) but not what the real board ships; the
  component-described profile fixes it.

Consequence for profiles: a byte-faithful Replica 1 mounts BASIC at
`$E000` and the Krusader `$F000-$FFFF` slice — **including Krusader's
own monitor page** — rather than Krusader plus a separate pristine
WozMon (which would change the `$FF00` bytes away from the real ROM).
A pristine-WozMon Replica 1 stays one overlay away for whoever wants
it.

## The embedded ROM set (`roms/`)

Name = file stem = `builtin:` token, mirroring the configs convention:

| File | Size | Mounts at | Contents |
|---|---|---|---|
| `WozMon.rom` | 256B | `$FF00` | pristine Woz Monitor (today's `apple1.rom`) |
| `apple1-basic.rom` | 4KB | `$E000` | Woz Integer BASIC (identical first 4K of every Krusader image) |
| `Krusader-1.3-6502.rom` | 4KB | `$F000` | Krusader 1.3 (6502 build), its monitor page included |
| `Krusader-1.3-65C02.rom` | 4KB | `$F000` | Krusader 1.3 (65C02 build), its monitor page included |

Provenance: Krusader is Ken Wessen's, distributed freely for the
Replica 1; Integer BASIC and the Woz Monitor are the same Apple
images the emulator has always shipped. A concatenation test pins the
decomposition: `apple1-basic.rom + Krusader-1.3-6502.rom` must equal
the historical 8KB `krusader.rom` byte-for-byte.

The only *fixed* hardware in the one family is the PIA at
`$D010-$D013`; everything else — CPU, RAM banks, ROM images — is
config. (The Apple 1's 7-bit display masking stays a `machine.model`
behavior: it is a property of the terminal section, not of the memory
map.)

## As built (plans/20260719-03, R1–R3)

- **`rom/` renamed to `roms/`**; the four mountable images landed as
  committed files (name = stem = `builtin:` token), the historical
  `apple1.rom`/`krusader.rom` retired, and the decomposition is pinned
  by SHA-1 provenance tests (`config.rs`) *and* machine-level byte
  identity (`one.rs`): the composed `builtin:replica1` `$E000-$FFFF`
  equals the Krusader 65C02 distribution, and the 6502-slice spelling
  reproduces the historical `krusader.rom` machine.
- **`config::rom_builtin` / `read_memory_image`**: a memory region's
  `path` of `builtin:<name>` resolves against the embedded registry,
  never the filesystem; `referenced_files` skips it, so built-in
  configs carry ROM references and stay self-contained.
- **`machine.cpu`** (`"6502"`/`"65C02"`) and **RAM-bank regions**
  (`size`, RAM only, exactly-one-of path/size) are apple1-family keys;
  layouts are validated — no overlaps with each other or the PIA, fit
  in 64K, and something must cover the reset vector (`$FFFC-$FFFD`).
  File images have unknown extents and get start-address checks plus
  benefit of the doubt on the vector.
- **`One::from_components(model, cpu, regions)`** is the single
  construction path (no base RAM — the board is entirely regions);
  `One::new(model)` builds from the model's *built-in config*, so the
  board is described exactly once, in `configs/`. One-family
  `machine.memory` describes the **whole board**; absent means the
  model's builtin (`normalize` fills the options, so `--print-config`
  always shows the full board).
- **The profiles are the maps above**: `builtin:apple1` = 6502 +
  4KB@`$0000` + BASIC preloaded as *writable RAM* in the `$E000` bank
  (cassette-faithful, minus the cassette; `E000R` starts it — pinned
  by a boot-to-BASIC test) + pristine WozMon; `builtin:replica1` =
  65C02 + 32KB + ROM BASIC + the 65C02 Krusader slice, fixing the
  historical 6502-build-on-65C02 mismatch.

## The tty / telnet frontend (plans/20260719-04, as built)

The Apple 1 is a terminal machine, so `ewm one --tty` runs it headless
with stdin/stdout as the keyboard and display — the local terminal,
`nc`, or (via `scripts/systemd/`, inetd-style `Accept=yes` socket
activation with `StandardInput=socket`) one fresh machine per telnet
connection on port 6502. The emulator does no networking.

- **Pacing**: 20 ms ticks throttled to 1.023 MHz wall-clock (unused
  tick budget is slept off, so a paste cannot sprint the machine);
  keys feed one at a time gated on the PIA's one-byte latch
  (`Pia::key_pending` — IRQA1, cleared when the ROM reads `$D010`).
- **Mapping**: a–z uppercase in; LF/CRLF → CR in; CR → CRLF out;
  7-bit printables only. Ctrl bytes and a bare ESC belong to the
  machine (real CTRL key; ESC is the monitor's cancel-line key).
- **Reset is Meta-R** — the keyboard's free modifier: a lone ESC is
  held ~50 ms for the `r` of `ESC r`; matched → warm reset, otherwise
  the ESC is forwarded. Telnet `BRK`/`IP` (`send brk`) also resets —
  immediately, flushing typed-ahead, like the real button.
- **Banner**: `--tty-banner <path>` prints a text file (CRLF-
  normalized) to the session before the machine says anything —
  visitor instructions; `scripts/systemd/banner.txt` is the shipped
  example, wired into the service unit.
- **Telnet** (hand-rolled RFC 854 subset): dormant until the first
  inbound `IAC`, so `nc` and local terminals never see protocol
  bytes; then `WILL ECHO` + `WILL SGA` are announced (character-at-a-
  time, remote echo), other negotiations are refused, subnegotiations
  swallowed. Input EOF gets a two-emulated-second grace so the last
  command finishes printing.
