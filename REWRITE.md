# EWM C-to-Rust Rewrite Plan

This is a working document for an iterative, agent-driven rewrite of EWM from C to Rust.
Re-read this file at the start of every session. Update the phase checklist as work
completes. **The tree must build and pass all verification gates after every phase.**

## Status

| Phase | Description | Size | Status |
|---|---|---|---|
| 0 | Workspace scaffolding + CI | S | Done |
| 1 | 6502 core (Dormann functional test) | L | Done |
| 2 | 65C02 + disassembler + golden traces | M | Done |
| 3 | Apple 1 / Replica 1, headless | M | Done |
| 4 | Apple 1 SDL frontend | M | Done (manual checklist below) |
| 5 | Apple ][+ machine, headless, no disk | L | Done |
| 6 | Disk II | L | Not started |
| 7 | Apple ][+ SDL frontend + boo menu | L | Not started |
| 8 | Parity sweep, benches, docs | M | Not started |
| 9 | Remove C, promote Rust to root | M | Not started |

## Ground rules (apply to every phase)

- **Parallel rewrite.** All Rust code lives under `rust/`. C code in `src/` is never
  modified until Phase 9. `cd src && make` must keep working through Phase 8.
- Each phase is one PR-sized unit: independently completable, verifiable with the
  commands listed in its gate, and leaves both C and Rust builds green.
- Port behavior, not just structure — including documented quirks (see "Quirks to
  preserve" below). When the C code and the 6502 spec disagree, match the C.
- No new emulation features during the port. Feature ideas go in "Future work" at the
  bottom of this file.
- If a phase gate fails at session end, revert to the last green commit rather than
  leaving a broken tree.

## Top-level decisions (decided; do not relitigate per phase)

| Decision | Choice | Rationale |
|---|---|---|
| Workspace location | Self-contained Cargo workspace in `rust/` (`rust/Cargo.toml` with members `ewm-core`, `ewm`) | Zero interference with the C build during the parallel period; Phase 9 is a mechanical `git mv` to the root. CI uses `--manifest-path rust/Cargo.toml`. |
| Crate structure | Two crates: `ewm-core` (no SDL dependency: CPU, bus, machines, devices, headless logic) and `ewm` (binary: SDL2 windowing, renderers, audio, input, menu, CLI) | The Dormann tests and machine integration tests run headless in CI; the crate boundary *enforces* the core/frontend seam instead of relying on discipline. |
| Memory system (C: `mem_t` linked list + fn pointers + `void *obj`) | `trait Bus { fn read(&mut self, addr: u16) -> u8; fn write(&mut self, addr: u16, b: u8); }`. `Cpu` holds **no memory** — `step`, `reset`, `irq`, `nmi` take `&mut dyn Bus`. Each machine struct (`One`, `Two`, `TestBus`) implements `Bus`, owns its RAM/ROM/devices as plain fields, and dispatches with a `match` on address ranges. Language-card switching becomes state fields consulted in `Two::read`/`write` instead of toggled region flags. | The `void *obj` seam becomes the trait object; the linked-list walk becomes a match (faster and clearer); and this dissolves the RAM-aliasing problem (next row). |
| RAM aliasing (C: renderers scan `cpu->ram` directly) | Machine owns `ram: Vec<u8>`; the CPU never holds a RAM pointer. Renderers take `&Two` (immutable) and are called between step batches in the frame loop — exactly the C sequencing, now borrow-checker-legal by construction. No `Rc<RefCell>`, no `unsafe`. | |
| Instruction dispatch | Port the C design 1:1: a 256-entry `[Instruction; 256]` table per model. `Instruction { name, bytes, cycles, handler: Handler }` where `Handler` is a 3-variant enum — `Implied(fn(&mut Cpu, &mut dyn Bus))`, `Byte(fn(.., u8))`, `Word(fn(.., u16))` — the type-safe version of C's arity-cast `void *`. 65C02 table = copy of the 6502 table with overrides overlaid at construction (mirrors the C back-fill), built once. | A mechanical 1:1 port of ~200 handlers is the lowest-risk path; a giant `match` can be a later refactor. `dyn Bus` dispatch is plenty fast for a 1 MHz target (Phase 8 benches confirm). |
| CPU flags | Keep the C layout: separate fields `n v b d i z c`, packed/unpacked only in `status()` / `set_status()` | Exact behavioral match (B-flag handling) and 1:1 handler porting. |
| ROM paths | `include_bytes!` for all machine/character ROMs (they are checked into the repo; the Disk II boot ROM is already a C array). During Phases 0–8, core references `../../src/rom/...` relative to the crate; Phase 9 moves `rom/` to the repo root and fixes the paths in one edit. Only user-supplied files (disk images, `--memory` files, trace paths) are runtime paths. Dormann test bins load at runtime via `env!("CARGO_MANIFEST_DIR")`-relative paths (test inputs, not assets). | Kills the "must run from `src/`" quirk; the Rust binary works from any cwd with no data files to install. |
| Trace comparison vs C | Golden-file based, not live lockstep. One-time procedure (documented script, run manually, not in CI): build C with tracing, capture the first ~100k instructions of the 6502 functional test as a normalized text trace (`PC A X Y SP P` per step), check in gzipped under `rust/ewm-core/tests/golden/`. A Rust test replays and diffs, reporting the first divergent instruction. | Golden files keep CI free of the C toolchain and survive C removal in Phase 9. The Dormann tests are the *pass* gate; the trace diff exists to *localize* failures (a Dormann failure alone gives a deadlock PC, not the culprit instruction). |
| CLI | Hand-rolled arg parsing in `main.rs` mirroring the existing `getopt_long` flags (`ewm one --model apple1\|replica1 --memory ... --trace ...`, `ewm two --color --fps N --drive1 ...`, `ewm boo`) — no clap. | Flag-for-flag parity is the parity contract; avoids a dependency and behavior drift. |
| Lua | Dropped. `lua.c`, `scripts/*.lua`, and all `EWM_LUA` paths are not ported. Listed under Future work (mlua). | Optional feature, threaded through cpu/two/dsk behind `#ifdef`; dropping it greatly simplifies the CPU dispatch port. |

## Module naming map

| C | Rust | Crate |
|---|---|---|
| `cpu.c/h` | `src/cpu.rs` (registers, step, stack, vectors, reset/irq/nmi) | ewm-core |
| `mem.c/h` | `src/bus.rs` (Bus trait, `TestBus` flat 64K); addressing-mode/RMW helpers move into `src/ins.rs` alongside their only callers | ewm-core |
| `ins.c/h` | `src/ins.rs` (tables + handlers) | ewm-core |
| `fmt.c/h` | `src/fmt.rs` (disassembler, trace-line formatter) | ewm-core |
| `pia.c/h` | `src/pia.rs` | ewm-core |
| `one.c` (machine half) | `src/one.rs` (`One` struct + Bus impl) | ewm-core |
| `two.c` (machine half) | `src/two.rs` (`Two` struct, soft switches, key latch, speaker/paddle state) | ewm-core |
| `alc.c/h` | `src/alc.rs` (language-card state, driven from `Two`'s bus) | ewm-core |
| `dsk.c/h` | `src/dsk.rs` (nibblization, boot ROM, stepper, IOM) | ewm-core |
| `chr.c` (bitmap half) | `src/chr.rs` (char ROM → glyph bitmaps) | ewm-core |
| `utl.c/h` | `src/util.rs` (file loading, misc) | ewm-core |
| `one.c` (SDL loop) | `src/one.rs` | ewm (bin) |
| `two.c` (SDL loop) | `src/two.rs` | ewm (bin) |
| `tty.c/h` | `src/tty.rs` | ewm |
| `scr.c/h` | `src/scr.rs` | ewm |
| `snd.c/h` | `src/snd.rs` (toggle events → samples → `queue_audio`) | ewm |
| `chr.c` (texture half) | `src/chr.rs` (glyph bitmaps → SDL textures) | ewm |
| `sdl.c/h` | `src/sdl.rs` (init/joystick helpers) | ewm |
| `boo.c/h` | `src/boo.rs` | ewm |
| `ewm.c` | `src/main.rs` (subcommand dispatch) | ewm |
| `cpu_test.c` | `ewm-core/tests/dormann.rs` | ewm-core |
| `cpu_bench.c` / `mem_bench.c` | `ewm-core/benches/` | ewm-core |
| `scr_test.c` / `tty_test.c` | manual SDL viewers — dropped; equivalent coverage via headless gates + Phase 7 screenshots | — |
| `lua.c/h` | not ported | — |

## Quirks to preserve (verify per phase, do not "fix")

1. IRQ/NMI push `pc + 1`, not the spec's `pc + 2` (`cpu.c` comments this; the Dormann
   tests depend on it).
2. Disk II writes are no-ops (write support stubbed in `dsk.c`).
3. The MHz display is fake (always ≈1.023 MHz, not measured).
4. `apple2` (non-plus) and `apple2e` machine types return an error, as in C.
5. Cycle budget per frame = `1023000 / fps`, default 40 fps fixed step
   (`EWM_TWO_SPEED`, `EWM_TWO_FPS_DEFAULT` in `two.h`).
6. CPU test success detection = branch-to-self deadlock check; start `$0400`,
   success PC `$3399` (6502) and `$24a8` (65C02), per `cpu_test.c`.

---

## Phase 0 — Workspace scaffolding + CI (S)

**Goal:** An empty but building Rust workspace beside the untouched C tree, with CI
guarding both.

**Scope:**
- `rust/Cargo.toml` (workspace), `rust/ewm-core` (lib with empty modules),
  `rust/ewm` (bin that prints usage), `rust/rust-toolchain.toml`, `rust/rustfmt.toml`.
- `.github/workflows/ci.yml` (new — the repo has no CI; `.travis.yml` is defunct):
  - Job 1: install `libsdl2-dev`, build the C tree (`cd src && make`).
  - Job 2: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, all with
    `--manifest-path rust/Cargo.toml`.
- Short "Rust rewrite in progress, see REWRITE.md" note in README.

**Key decisions:** `ewm-core` has zero non-dev dependencies; `ewm` declares the `sdl2`
crate now so CI installs SDL2 headers from day one.

**Gate:** CI green on both jobs; `cargo run --manifest-path rust/Cargo.toml -p ewm`
prints usage; `cd src && make` still builds.

## Phase 1 — 6502 core, gated on the Dormann functional test (L)

**Goal:** A complete NMOS 6502 with the Bus trait, passing the Klaus Dormann
functional test.

**Scope:**
- Port `cpu.c` → `cpu.rs`, `mem.c` → `bus.rs` (+ helpers into `ins.rs`), and the 6502
  half of `ins.c` (table + all handlers) → `ins.rs`.
- `ewm-core/tests/dormann.rs` ports the `cpu_test.c` harness: load
  `src/rom/6502_functional_test.bin` into a flat-64K `TestBus` at `$0000`, set
  PC=`$0400`, step until PC==`$3399` (pass) or PC repeats (fail — print the PC).

**Key decisions:** All the top-level decisions land here: Bus trait, `Handler` arity
enum, separate flag fields, the pc+1 interrupt quirk. Port handlers in the same order
as `ins.c`'s table so diffs against C are positional. Port `strict` mode (stack
over/underflow checks) as a `Cpu` flag.

**Gate:** `cargo test -p ewm-core` passes `dormann_6502` (PC reaches `$3399`). Unit
tests for status pack/unpack round-trip and stack wraparound.

## Phase 2 — 65C02, disassembler, golden-trace harness (M)

**Goal:** 65C02 mode passes the extended-opcodes test, and any divergence from C is
mechanically localizable.

**Scope:**
- 65C02 override handlers + table overlay in `ins.rs` (BRA, STZ, PHX/PLX/PHY/PLY,
  SMB/RMB/BBR/BBS, TRB/TSB, decimal-corrected ADC/SBC, `(zp)` modes).
- `fmt.c` → `fmt.rs` (disassembly + trace-line formatting); `Cpu` trace output in the
  same format as C.
- `scripts/gen-golden-trace.sh` (documented one-time C-side capture) + checked-in
  gzipped ~100k-instruction golden trace + `ewm-core/tests/trace_compare.rs`.

**Key decisions:** Golden files over live lockstep (see decisions table). Normalize
the trace format (uppercase hex, fixed columns) in the capture script, not in the
emulators.

**Gate:** `cargo test -p ewm-core` passes `dormann_65c02` (PC reaches `$24a8`) and
`trace_compare` (zero divergence over the golden window).

## Phase 3 — Apple 1 / Replica 1, headless (M)

**Goal:** A bootable headless Apple 1: Woz monitor prompt reachable and interactive
through the PIA.

**Scope:**
- `pia.c` → `pia.rs`: 6820 PIA at `$D010`; the C output callback becomes an output
  byte sink the frontend (or test) drains.
- `one.c` machine half → `ewm-core/src/one.rs`: `One::new(model)` wires RAM + ROMs
  (`apple1.rom`, `krusader.rom` via `include_bytes!`) and implements `Bus`.
- `utl.c` → `util.rs`.

**Key decisions:** Keyboard input = `One::key(u8)` pushing into the PIA (with the
7-bit masking `one.c`'s callback applies); display output = PIA output bytes appended
to a buffer that the test (later: the tty renderer) drains — the same callback seam
as C, minus SDL.

**Gate:** `ewm-core/tests/one_boot.rs`: create an Apple 1, reset, step ~1M cycles,
feed `E000.E00F\r` to the Woz monitor, assert the drained output contains the
expected hex dump of that ROM region. A second test echoes a typed character
(mirrors `tests/apple1/echo.s` intent).

## Phase 4 — Apple 1 SDL frontend (M)

**Goal:** `ewm one` runs windowed with keyboard and terminal display, matching the C
binary side by side.

**Scope:**
- `chr.c` bitmap half → `ewm-core/src/chr.rs` (ROM `3410036.bin` → glyph bitmaps);
  texture creation → `ewm/src/chr.rs`.
- `tty.c` → `ewm/src/tty.rs`; `one.c` SDL loop → `ewm/src/one.rs`; `sdl.c` →
  `ewm/src/sdl.rs`.
- CLI subcommand `one` with `--model`/`--memory`/`--trace` in `main.rs`.

**Key decisions:** Glyph decoding is core (unit-testable, no SDL); textures are
frontend. The frame loop copies the C loop structure (event pump → CPU cycle budget →
tty render) rather than inventing a new timing scheme.

**Gate:** Unit test: a known glyph ('A') decodes to the expected bitmap. Manual
checklist (record results in this file): `cargo run -p ewm -- one --model replica1`
shows the prompt; typing echoes; Krusader assembles a two-line program; behavior
matches `src/ewm one` run alongside.

**Manual checklist results:**
- [x] Unit tests: 'A' glyph bitmap, inverse glyphs, unmapped codes (`chr.rs`)
- [x] `cargo run -p ewm -- one --model replica1` launches, renders, runs, and
  exits cleanly (smoke-tested 4s)
- [ ] Prompt shows and typing echoes (verify by hand)
- [ ] Krusader assembles a two-line program
- [ ] Behavior matches `src/ewm one` run alongside

## Phase 5 — Apple ][+ machine, headless, no disk (L)

**Goal:** A headless Apple ][+ that boots the ROMs into AppleSoft and evaluates BASIC,
verified by text-page inspection.

**Scope:**
- `two.c` machine half → `ewm-core/src/two.rs`: RAM `$0000–$BFFF`, ROMs at
  `$D000–$FFFF`, full `$C000–$C07F` soft-switch dispatch (keyboard latch/strobe,
  TEXT/MIXED/PAGE2/HIRES, speaker toggle recording at `$C030`, annunciators, buttons
  `$C061–$C063`, paddle trigger/read `$C070`/`$C064–$C067` as settable fields),
  screen-mode state.
- `alc.c` → `alc.rs`: language-card bank state consulted by `Two::read`/`write` for
  `$D000–$FFFF` and the `$C080–$C08F` switch reads.

**Key decisions:** Speaker = a vector of cycle-stamped toggle events recorded on
`$C030` access, drained by the frontend later (keeps sound out of core). A text-page
scraping helper `Two::text_screen() -> String` (decode the `$0400` page using the
interleaved row offsets and charset) lives in core — it is the workhorse for every
headless gate from here on.

**Gate:** `ewm-core/tests/two_boot.rs`: boot, step until `text_screen()` contains the
`]` prompt (with a cycle cap); type `PRINT 2+2\r` via the key latch; assert `4`
appears. Language-card tests: exercise `$C08x` sequences and assert read/write
banking matches the C semantics (including the double-read write-enable behavior).

## Phase 6 — Disk II (L)

**Goal:** Boot DOS 3.3 from a `.dsk` image, fully headless.

**Scope:**
- `dsk.c` → `ewm-core/src/dsk.rs`: nibblization of `.dsk`/`.do`/`.po`/`.nib` images
  to GCR 6-and-2 on load (sector interleave tables, address/data field encoding),
  the embedded 256-byte boot ROM at `$C600` (const array), `$C0E0–$C0EF` IOM handling
  in `Two`'s bus dispatch, stepper phase→track math (`dsk_phase_delta`).
- Writes remain no-ops (quirk #2).

**Key decisions:** The nibblizer gets its own unit tests (interleave tables,
prologue/checksum bytes) since it is the most algorithmic, least observable code in
the repo. Drive selection/mode flags mirror C exactly.

**Gate:** Unit tests: nibblize a known sector and assert the address-field prologue,
volume/track/sector, and checksum bytes. Integration `ewm-core/tests/two_dos.rs`:
insert `disks/DOS33-SystemMaster.dsk`, boot, step until `text_screen()` shows the DOS
banner and `]`; type `CATALOG\r`; assert known filenames (e.g. `HELLO`) appear.

## Phase 7 — Apple ][+ SDL frontend + boo menu (L)

**Goal:** Full windowed `ewm two` (and `ewm boo`) at feature parity with the C binary.

**Scope:**
- `scr.c` → `ewm/src/scr.rs`: 280×192 renderer, TEXT (interleaved row offsets), LGR
  (16-color), HGR (including the color-fringing fix from #187), mixed mode, page 2,
  flashing text via the frame-phase counter, `--color`/green/white schemes.
- `snd.c` → `ewm/src/snd.rs`: drain the core's speaker toggle events → sample buffer
  → SDL `queue_audio` (port of #188).
- `two.c` SDL loop → `ewm/src/two.rs`: 40 fps fixed step, `1023000/fps` cycle budget,
  joystick→paddles, key mapping, Cmd-Esc reset / Cmd-Return fullscreen / Cmd-P pause /
  Cmd-I status bar, status-bar tty overlay.
- `boo.c` → `ewm/src/boo.rs`; complete `main.rs` dispatch (`one`/`two`/`boo`, no args
  → boo; fake MHz display preserved).

**Key decisions:** The renderer reads `&Two` between step batches (the aliasing
decision realized). Add a hidden `--screenshot <path>` debug flag (dump the render
surface as BMP after N frames) so graphics gates are automatable and comparable
against the C build.

**Gate:** Automated: `--screenshot` after booting System Master matches a checked-in
golden BMP of the text screen. Manual checklist recorded here: boot System Master to
`]`, `CATALOG`, `RUN` a program from `DOS33-SamplePrograms.dsk` exercising LGR and
HGR, speaker beeps on boot, paddles respond if a joystick is present.

## Phase 8 — Parity sweep, benches, docs (M)

**Goal:** Nothing left that the C binary does and the Rust one doesn't (except Lua).

**Scope:**
- Port `cpu_bench`/`mem_bench` as `ewm-core/benches/` (plain `std::time` harness, no
  criterion dependency, matching the C ops/sec output).
- CLI flag audit against the `getopt_long` tables in `one.c`/`two.c`/`ewm.c`;
  `--trace`/`--strict` wired end to end.
- README gains a "Rust build" section; this file gains a filled-in parity checklist
  (flag by flag, feature by feature).

**Key decisions:** Benches are informational, not gated; record C-vs-Rust numbers in
this file. Any deliberate divergence found in the audit gets a line added to "Quirks
to preserve" — parity is *documented*, not assumed.

**Gate:** Every row of the parity checklist marked done or explicitly waived; all
prior automated gates still green; bench numbers recorded.

## Phase 9 — Remove C, promote Rust to root (M)

**Goal:** A single-language repo: Rust is EWM.

**Scope:**
- Delete `src/*.c`, `src/*.h`, `src/Makefile`, `src/CMakeLists.txt`, `.travis.yml`,
  `scripts/*.lua`.
- `git mv` the workspace to the root (root `Cargo.toml`); `git mv src/rom rom` and fix
  the `include_bytes!`/test paths.
- Keep `disks/` and `tests/` (cc65 asm sources; mark their harnesses as manual).
- CI drops the C job. README rewritten: build = `cargo build --release`, run =
  `cargo run --release -- two --color --drive1 disks/DOS33-SamplePrograms.dsk`.

**Key decisions:** Do the move as pure `git mv` + path fixes in one commit and any
remaining edits in separate commits — this makes review of the destructive phase
trivial.

**Gate:** From a fresh clone: `cargo test` green (Dormann ×2, trace_compare,
one_boot, two_boot, two_dos), `cargo run -- boo` boots both machines, System Master
boots to `]` and runs `CATALOG`; no `.c`/`.h` files remain; README instructions
verified by following them literally.

---

## Sequencing notes

- Phases are strictly ordered 0→9. Phase 4 may be deferred until after 5/6 if
  headless momentum is preferred; nothing else reorders.
- The golden-trace harness (Phase 2) is the debugging tool of first resort for any
  CPU-level regression discovered in later phases.

## Future work (out of scope for the port)

- Lua scripting via `mlua` (re-expose the cpu/machine hooks `lua.c` had).
- Disk II write support; `.woz` image support.
- Apple II (non-plus) and Apple IIe machine models.
- Real measured MHz display; configurable speed throttling.
- A debugger (listed in README goals, never implemented in C).
