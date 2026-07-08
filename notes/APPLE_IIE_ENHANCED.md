# Apple //e Enhanced Support — Implementation Plan

A working document for adding the **Enhanced Apple //e** to EWM as a third
machine alongside the Apple 1 / Replica 1 (`one`) and the Apple ][+ (`two`).
Like `REWRITE.md`, this is meant to be re-read at the start of every session
and updated as phases land. **The tree must build and pass all verification
gates (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`) after every phase.**

The work is deliberately sliced into many small, PR-sized phases. Each phase
is independently completable, leaves the existing Apple ][+ path
byte-for-byte unchanged, and adds one observable capability to the //e.

## Why the Enhanced //e specifically

The "Enhanced" //e is the sweet spot for EWM because most of its hardest
parts already exist:

- **65C02 CPU** — the Enhanced //e swapped the NMOS 6502 for the 65C02.
  `ewm-core` already ships a fully-tested 65C02 (`Model::M65C02`,
  `instructions_65c02()`, gated by the Dormann extended-opcodes test). No CPU
  work is required.
- **Language card built in** — the //e has the ][+ language card soldered
  onto the board. `ewm/src/alc.rs` already implements it; the only change is
  making its `$D000-$FFFF` banking aware of the new **ALTZP** aux switch.
- **Disk II / hard drive / clock / sound / tty** — all reused unchanged
  (`dsk.rs`, `hdd.rs`, `clk.rs`, `snd.rs`, `tty.rs`).

What is genuinely new: **auxiliary memory + the MMU/IOU soft switches**,
**80-column text**, **double-lo-res / double-hi-res graphics**, the
**enhanced character ROM** (lower case + MouseText + alternate character
set), and the **//e keyboard** (lower case, Open/Solid-Apple keys).

We target the *Enhanced* //e only. The original NMOS //e and the //c are out
of scope (see "Future work").

## Status

| Phase | Description | Size | Status |
|---|---|---|---|
| 0 | Plan, ROM assets, and the //e memory/soft-switch map | S | Not started |
| 1 | Enhanced character ROM → glyph tables (pure, unit-tested) | M | Not started |
| 2 | 64K //e machine: 65C02 + //e ROMs, boots headless to `]` in 40 cols | L | Not started |
| 3 | //e 40-column display + keyboard (lower case, MouseText, Apple keys) | M | Not started |
| 4 | Auxiliary memory + MMU routing (RAMRD/RAMWRT/ALTZP/80STORE) | L | Not started |
| 5 | 80-column text display (main+aux interleave, 560-wide buffer) | L | Not started |
| 6 | Double-lo-res + double-hi-res graphics (DHIRES/AN3) | L | Not started |
| 7 | SDL frontend, boo menu, and CLI wiring | M | Not started |
| 8 | Parity sweep, self-test, ProDOS 80-col, docs | M | Not started |

## Ground rules (apply to every phase)

- **The Apple ][+ is frozen.** Every existing gate — `two_boot`, `two_dos`,
  `two_hdd`, `two_clk`, `two_timing`, and the `boot_screen_matches_golden_bmp`
  golden screenshot — must stay green and unchanged. The //e is additive.
- Each phase is one PR-sized unit with the gate commands listed in its
  section. If a gate fails at session end, revert to the last green commit
  rather than leaving a broken tree.
- Prefer headless, deterministic gates (`text_screen()`-style RAM scrapes and
  golden BMPs) over manual checks, exactly as the rewrite did. Manual
  checklists are recorded in-file per phase for the windowed pieces.
- `ewm-core` stays a **generic** 6502 kernel. No Apple //e concept (aux
  memory, soft switches, MMU state) may leak into `cpu.rs` / `mem.rs` /
  `ins.rs` / `fmt.rs`. All //e-specific banking lives in the `ewm` crate,
  following the existing `Alc`-as-`Device` precedent.
- Port *observable behavior*. Where real //e hardware has murky/undocumented
  behavior (open-bus reads on status switches, RMW double-writes to soft
  switches), pick the simple faithful choice and record it under "Quirks &
  divergences" rather than chasing bug-for-bug fidelity with silicon.

## Top-level decisions (decided up front; do not relitigate per phase)

| Decision | Choice | Rationale |
|---|---|---|
| Target model | **Enhanced //e** only (65C02 + enhanced ROM set + MouseText char ROM). Reuse the existing `TwoType::Apple2E` enum variant to mean "Enhanced //e"; leave `TwoType::Apple2` erroring (out of scope). | The 65C02 is already done and tested; the enhanced ROM is what most //e software assumes. One target keeps the matrix small. |
| Machine structure | **Extend the existing `Two` / `two.rs`** with a `model: TwoType` field rather than forking a new `TwoE`. The ][+ construction path is untouched; //e-specific devices, ROMs, char set, and renderer paths are selected on `model`. | Disk II, HDD, clock, sound, tty, palette, and the frame loop are all shared verbatim. One evolving machine file keeps PRs focused. Split into `two_e.rs` later only if the file grows unwieldy. |
| Where banking lives | Auxiliary memory + the MMU/IOU soft switches live in the **`ewm` crate as a `Device`** (working name `Mmu`/`IouE`), the same pattern as `Alc`. The //e's `Memory` is built with **no base-RAM fast path** (`Memory::new(0)`) so *all* RAM (incl. zero page and stack) flows through the aux-aware device. | Zero changes to `ewm-core`. Reuses a proven seam. Per-access `dyn Device` dispatch is far inside the 1 MHz (even 7.16 MHz accelerated) budget — the rewrite benches already proved `ind*`-style dispatch is ~200 ms / 100M ops. |
| Aux/switch state sharing | A **single //e memory-management device owns everything the MMU/IOU arbitrate**: main RAM, aux RAM, and the `$C000-$C01F` soft switches. It is mapped over `$0000-$BFFF`, `$C000-$C01F`, and (ALTZP-aware) shares the language-card region. The Disk II / clock / HDD keep their own `$C0Ex/$C09x/$C0Fx` sub-ranges and shadow it via the newest-first region walk (as `Dsk` already shadows `TwoIo` today). | The real MMU/IOU are the central arbiters; centralizing their state avoids `Rc<RefCell>` (which the project deliberately avoids) and matches the hardware. |
| Renderer resolution | The //e renders into a **560×192** internal buffer: 80-column text and double-hi-res are native 560; 40-column / LGR / HGR pixels are drawn 2× horizontally. The ][+ keeps its **280×192** `Scr` path and golden test untouched. | 80-col and DHGR are inherently double-horizontal-res. A separate //e render path avoids disturbing the ][+ golden BMP. Unifying the two renderers is optional later cleanup. |
| ROM assets | Check the Enhanced //e ROMs into `rom/`, same stance as the existing `341-00xx` ][+ ROMs: **342-0304-A** (CD, `$C100-$DFFF` region) + **342-0303-A** (EF, `$E000-$FFFF`) system ROM, and **342-0265-A** enhanced US video/character ROM (primary + alternate/MouseText sets). Load via `include_bytes!`. | Consistent with how EWM already ships machine + character ROMs. Part numbers are guidance; the actual dumps must be sourced and their sizes/hashes verified in Phase 0. |
| Memory size | Default the //e to **128K** (64K main + 64K aux, i.e. the Extended 80-Column Text Card fitted). The 64K "no aux card" config is a real intermediate milestone (Phase 2) but the shipped machine is 128K. | 80-col, DHGR, and virtually all //e software assume the extended card. |
| 65C02 timing | Reuse the current 65C02 instruction timings. The //e-specific cycle deltas (decimal-mode +1, fixed page-cross reads) stay **out of scope**, consistent with the rewrite's existing note that "65C02-specific timing remains out of scope." | Keeps parity with the decision already recorded in `REWRITE.md`; the display/boot gates compare architectural state, not cycles. |

## The //e memory map & soft switches (reference)

This table is the contract Phases 4–6 implement. Addresses are the standard
//e I/O locations. "R" = read triggers/reports, "W" = write triggers.

### Memory-management soft switches (`$C000-$C00F`, write to set)

| Off | On | Name | Effect |
|---|---|---|---|
| `$C000` W | `$C001` W | 80STORE | When on, PAGE2 routes text page 1 (`$0400-$07FF`) — and, with HIRES on, hi-res page 1 (`$2000-$3FFF`) — to aux, overriding RAMRD/RAMWRT |
| `$C002` W | `$C003` W | RAMRD | Reads of `$0200-$BFFF` come from aux (on) or main (off) |
| `$C004` W | `$C005` W | RAMWRT | Writes to `$0200-$BFFF` go to aux (on) or main (off) |
| `$C006` W | `$C007` W | SLOTCXROM/INTCXROM | Select peripheral-slot ROM vs internal ROM at `$C100-$CFFF` |
| `$C008` W | `$C009` W | ALTZP | Zero page, stack (`$0000-$01FF`) and language-card RAM come from aux (on) or main (off) |
| `$C00A` W | `$C00B` W | SLOTC3ROM | Slot-3 ROM (`$C300`) vs internal 80-col firmware |
| `$C00C` W | `$C00D` W | 80COL | 80-column video (on) vs 40 (off) |
| `$C00E` W | `$C00F` W | ALTCHARSET | Alternate character set (MouseText/lower-inverse) vs primary |

### Status reads (`$C010-$C01F`, read returns switch state in bit 7)

| Addr | Name | Reports |
|---|---|---|
| `$C010` R | KBDSTRB / AKD | Clears the keyboard strobe; bit 7 = any-key-down |
| `$C011` R | RDLCBNK2 | Language-card bank 2 selected |
| `$C012` R | RDLCRAM | Language-card RAM read-enabled |
| `$C013` R | RDRAMRD | RAMRD state |
| `$C014` R | RDRAMWRT | RAMWRT state |
| `$C015` R | RDCXROM | INTCXROM state |
| `$C016` R | RDALTZP | ALTZP state |
| `$C017` R | RDC3ROM | SLOTC3ROM state |
| `$C018` R | RD80STORE | 80STORE state |
| `$C019` R | RDVBL | Vertical-blank (see Quirks) |
| `$C01A` R | RDTEXT | TEXT mode |
| `$C01B` R | RDMIXED | MIXED mode |
| `$C01C` R | RDPAGE2 | PAGE2 state |
| `$C01D` R | RDHIRES | HIRES mode |
| `$C01E` R | RDALTCHAR | ALTCHARSET state |
| `$C01F` R | RD80COL | 80COL state |

### Display & misc (mostly reuse the existing ][+ `TwoIo` handlers)

`$C050-$C057` TEXT/MIXED/PAGE2/HIRES (already handled), `$C05E`/`$C05F`
**DHIRES** on/off (double-res enable, interacts with AN3), `$C07E`/`$C07F`
**IOUDIS**/RDIOUDIS, `$C061-$C063` Open-Apple / Solid-Apple / Shift buttons
(already read as buttons), `$C080-$C08F` language card (already handled by
`Alc`, extend for ALTZP).

## Current architecture — what each phase touches

- `ewm-core/src/{cpu,mem,ins,fmt}.rs` — generic kernel. **Untouched** by this
  work (65C02 already present). If a tiny accessor is unavoidable (e.g. to let
  the renderer see aux RAM) it must remain Apple-agnostic.
- `ewm/src/two.rs` — the machine + SDL loop. Gains a `model` field, //e ROM
  loading, the `Mmu` device wiring, and //e branches in the frame loop.
- `ewm/src/alc.rs` — language card. Gains ALTZP awareness (aux bank of LC RAM).
- `ewm/src/chr.rs` — character generator. Gains the enhanced 4K char ROM
  decode (primary + alternate sets, lower case, MouseText).
- `ewm/src/scr.rs` — renderer. Gains a 560-wide //e path: 80-col text, DLGR,
  DHGR, ALTCHARSET selection.
- `ewm/src/boo.rs`, `ewm/src/main.rs` — menu + CLI, gain the //e entry.
- Tests: new `ewm/tests/two_e_*.rs`; new `rom/` and possibly `disks/` assets.

---

## Phase 0 — Plan, ROM assets, and the memory/soft-switch map (S)

**Goal:** This document exists (done), the Enhanced //e ROMs are sourced and
checked in, and the machine map above is verified. No behavior change.

**Scope:**
- Source and add to `rom/`: the Enhanced //e system ROM (CD `342-0304-A` +
  EF `342-0303-A`, or an equivalent 16K `apple2e_enhanced.rom`) and the
  enhanced video ROM `342-0265-A`. Record exact byte sizes and SHA-256 hashes
  in this file so future contributors can verify their dumps.
- Confirm `TwoType::Apple2E` is the intended Enhanced-//e handle; leave
  `Two::new(TwoType::Apple2E)` returning its current error until Phase 2.
- Add a `.gitignore`/asset note if any ROM cannot be redistributed (fallback:
  document the expected filename and let the build `include_bytes!` fail with
  a clear message).

**Gate:** Tree builds, all existing tests green, ROMs present with recorded
hashes. `git grep -n Apple2E` shows the plumbing points a human will touch.

**Human dependency:** ROM dumps are copyrighted Apple firmware. The agent
cannot fetch them; a maintainer drops the verified files into `rom/`. Every
later phase that `include_bytes!`-loads them depends on this.

## Phase 1 — Enhanced character ROM → glyph tables (M)

**Goal:** A pure, unit-tested `Chr`-style decoder for the enhanced 4K
character ROM, independent of any machine — the same "decode is core,
textures are frontend" split the ][+ char ROM uses.

**Scope:**
- The enhanced video ROM holds **two** 128-glyph sets (primary and
  alternate) at 8 bytes/glyph. Decode both into `[Option<Glyph>; 256]`
  tables: primary (upper/lower case + inverse + flashing) and alternate
  (upper/lower + inverse + **MouseText** in `$40-$5F`).
- Note the //e char ROM layout differs from the ][+ 2716 dump; the current
  `chr.rs` `rom[c*8 + y + 1]` one-byte offset is specific to that part and
  must be re-derived for the //e ROM (likely no `+1`).
- Keep `Chr` (the ][+ path) intact; add the //e tables behind a constructor
  like `Chr::new_iie()` returning primary+alternate glyph sets, or a small
  `CharSet` enum the renderer selects with ALTCHARSET.

**Key decisions:** Lower case renders as real glyphs (not blanks). MouseText
occupies alternate-set codes `$40-$5F`. Flashing exists only in the primary
set; the alternate set replaces the flashing range with MouseText (the //e's
actual behavior).

**Gate:** Unit tests: a known upper-case glyph ('A'), a lower-case glyph
('a', which the ][+ set could not render), an inverse glyph, and a MouseText
glyph (e.g. the "open-apple") each decode to their expected bitmaps. No
machine, no SDL.

## Phase 2 — 64K //e machine: boots headless to `]` in 40 columns (L)

**Goal:** `Two::new(TwoType::Apple2E)` builds a 65C02 //e with the //e ROMs
and 64K of RAM that boots the ROM to the AppleSoft `]` prompt, verified by a
headless `text_screen()` scrape — the //e analogue of the Phase 5 rewrite
gate. No aux memory yet (a legitimate "no extended card" //e).

**Scope:**
- `Two::new` branches on `model`: for `Apple2E` build the CPU with
  `Model::M65C02`, load the //e system ROM into the language-card region
  (reuse `Alc`), and map the **internal `$C100-$CFFF` ROM** (self-test +
  80-col firmware + `$C800` shared expansion ROM).
- A minimal `Mmu`/`IouE` device covering `$C000-$C01F`: the write switches
  update MMU state (stored, but aux physically absent → aux reads fall back
  to main for now); the read switches at `$C011-$C01F` report state in bit 7;
  `$C010` clears the strobe and reports AKD. Reuse the existing `$C050-$C057`
  display switches from `TwoIo`.
- Internal-vs-slot ROM arbitration for `$C100-$CFFF` (SLOTCXROM/INTCXROM,
  SLOTC3ROM) at least far enough for the ROM's own cold-start path.
- Keep the ][+ `TwoIo` for the `Apple2Plus` model; the //e uses the new
  device. Slot 6 Disk II, slot 7 HDD, slot 1 clock still attach the same way.

**Key decisions:** The 64K milestone is real hardware, so aux switches are
inert (main answers) and documented as such. The //e boots to 40-column
Applesoft; `PR#3` (80-col) is expected to fail until Phase 5.

**Gate:** `ewm/tests/two_e_boot.rs`: construct `Apple2E`, insert
`DOS33-SystemMaster.dsk`, reset, step until `text_screen()` shows `]`; type
`PRINT 2+2` via the key latch; assert `4` appears. (DOS 3.3 runs on a //e.)
`two_boot`'s `apple2e_is_unsupported` assertion is updated/removed.

## Phase 3 — //e 40-column display polish + keyboard (M)

**Goal:** The windowed //e is usable in 40 columns: lower-case input and
display, MouseText via ALTCHARSET, and the Open/Solid-Apple keys.

**Scope:**
- Renderer: a //e 40-column text path that picks the primary or alternate
  glyph set from ALTCHARSET (`$C01E`), so software that flips to MouseText or
  relies on the //e's inverse-lower-case behavior renders correctly. Still in
  the 280-wide equivalent (pixel-doubled into the 560 buffer, or a dedicated
  40-col draw) — 80 columns is Phase 5.
- Keyboard: stop force-upper-casing input for the //e (the ][+ path
  upper-cases in `two.rs`); pass lower case through. Map the left/right GUI
  (or a configurable pair) to **Open-Apple** (button 0) and **Solid-Apple**
  (button 1), which the //e reads at `$C061`/`$C062` — needed for the ROM
  self-test and many games.
- ALTCHARSET/80COL/etc. state already lands in the `Mmu` from Phase 2; here
  the renderer starts *consuming* it.

**Key decisions:** Upper-casing stays for the ][+ (its ROM has no lower
case). Apple-key mapping is a documented divergence from the physical
keyboard (macOS reserves some Cmd combos, per the existing Cmd-Esc note).

**Gate:** Unit/headless: with ALTCHARSET on, a text page byte in `$40-$5F`
scrapes to its MouseText glyph; typing a lower-case line and scraping shows
lower case (a //e-aware `text_screen()` that preserves case). Manual
checklist recorded in-file: lower case echoes; a MouseText demo renders;
Open-Apple reads pressed.

## Phase 4 — Auxiliary memory + MMU routing (L)

**Goal:** The Extended 80-Column Card: a second 64K aux bank with full MMU
routing, so software can bank aux memory and read it back. This is the
architectural heart of //e support.

**Scope:**
- The `Mmu` device owns `main: [u8; 0x10000]`-worth and `aux: …` banks (or
  `$0000-$BFFF` + the LC region for each). Build the //e `Memory` with
  `Memory::new(0)` so **all** RAM flows through it.
- Implement the routing truth table exactly:
  - `$0000-$01FF` (ZP + stack) and the language-card RAM follow **ALTZP**.
  - `$0200-$BFFF` reads follow **RAMRD**, writes follow **RAMWRT**.
  - **80STORE** special case: when on, PAGE2 routes text page 1
    (`$0400-$07FF`) to aux regardless of RAMRD/RAMWRT; additionally with
    HIRES on, hi-res page 1 (`$2000-$3FFF`) too.
- Extend `Alc` (or fold LC into `Mmu`) so the `$D000-$FFFF` language-card RAM
  has main and aux copies selected by ALTZP.
- Renderer/machine accessors: `Two` exposes `main_ram()` and `aux_ram()` (or
  targeted page slices) so the 80-col/DHGR renderers can read both banks
  without violating the borrow rules — the same "renderer reads `&Two`
  between step batches" discipline the ][+ uses.

**Key decisions:** Keep it a single device to avoid shared-mutable-state
gymnastics. The `base_ram_size = 0` construction means zero page and stack
now dispatch through `dyn Device` on every push/pull — acceptable per the
budget analysis in the top-level decisions.

**Gate:** `ewm/tests/two_e_aux.rs` — a truth-table unit test mirroring the
//e MMU: for each combination of RAMRD/RAMWRT/ALTZP/80STORE/PAGE2/HIRES,
write a sentinel and assert it lands in (and reads back from) the correct
bank. A functional test: a short 65C02 program using the ROM `AUXMOVE`
(`$C311`) / `XFER` primitives round-trips a buffer main↔aux. `RDRAMRD` and
friends reflect the state that was set.

## Phase 5 — 80-column text display (L)

**Goal:** `PR#3` / 80COL produces real 80-column text, verified headless.

**Scope:**
- The //e renderer targets a **560×192** buffer. 80-column text reads
  interleaved memory: aux holds even columns (0,2,4,…), main holds odd
  columns (1,3,5,…), each glyph 7 px wide → 560 px. 40-column and LGR/HGR
  modes pixel-double into the same buffer.
- Drive it from 80COL (`$C00C`/`$C00D`) and 80STORE/PAGE2 (Phase 4 routing
  already puts the aux half in place). ALTCHARSET selects the glyph set
  (Phase 1/3).
- A `text_screen_80()` scrape (48×80? no — 24×80) reading both banks, the
  headless workhorse for this and later gates.
- SDL loop: the //e uses a 560-wide streaming texture and adjusts the window
  logical size / status-bar geometry accordingly (Phase 7 finishes the
  windowing; here the render buffer is correct and screenshot-testable).

**Key decisions:** Aux-even / main-odd interleave is the standard //e
convention; verify against the ROM's own 80-col output rather than assuming.
The ][+ 280-wide path and its golden BMP are not touched.

**Gate:** `ewm/tests/two_e_80col.rs`: boot, enable 80 columns (via `PR#3`
from Applesoft, or by poking the switches + firmware), print a known string,
and assert `text_screen_80()` shows it across 80 columns. A checked-in golden
560-wide BMP (`ewm/golden/two-e-80col.bmp`) via the `--screenshot` path.

## Phase 6 — Double-lo-res + double-hi-res graphics (L)

**Goal:** DLGR and DHGR render correctly.

**Scope:**
- **DHIRES** (`$C05E` on / `$C05F` off) plus AN3 and 80COL gate double-res.
  When active, hi-res reads interleave aux (even 7-px groups) and main (odd)
  for 560 horizontal pixels; the 4-bit-per-pixel //e color interpretation
  replaces the ][+ NTSC-fringing approximation.
- **DLGR**: double-width lo-res, aux/main interleaved, using the LGR color
  table already in `scr.rs`.
- `$C07E`/`$C07F` **IOUDIS** interaction: IOUDIS off exposes DHIRES at
  `$C05E`/`$C05F`; on, those addresses revert to AN3 control. Implement the
  documented precedence.
- Both color and monochrome //e schemes (the ][+ green/white/color schemes
  extend naturally).

**Key decisions:** DHGR color is a fresh implementation (4-bit patterns), not
a reuse of the ][+ single-hi-res fringing code. Keep a monochrome 560-wide
path for the golden test (deterministic, no NTSC guesswork).

**Gate:** `ewm/tests/two_e_dhgr.rs`: load a known DHGR bit pattern into
main+aux hi-res pages, enable DHIRES, render, and assert the 560-wide buffer
matches a golden BMP. A DLGR smoke render likewise.

## Phase 7 — SDL frontend, boo menu, and CLI wiring (M)

**Goal:** Launch the //e like any other machine: windowed, from the menu, and
from the command line.

**Scope:**
- `ewm/src/two.rs` `main`: select the model (new `--model 2e`/`//e` flag on
  `two`, or a dedicated path), size the window/texture for 560-wide output,
  title "EWM v0.1 / Apple //e", wire the 560 renderer, status bar, and the
  Apple-key mapping. Default the //e to 128K.
- `ewm/src/boo.rs`: add a menu entry (option **4**) "APPLE //e —
  65C02 / 128K / ENHANCED" and return a new `BooChoice::BootApple2E`.
- `ewm/src/main.rs`: dispatch the boo choice and any new subcommand/flag;
  update `usage()`.
- Command palette: the //e reuses the speed and pause/reset commands; add a
  "40/80 column" toggle if convenient.

**Key decisions:** Reuse the existing frame loop; branch only on render width
and model. A `--model` flag on `two` is less churn than a whole new
subcommand, and mirrors `one --model apple1|replica1`.

**Gate:** Automated: `ewm two --model 2e --screenshot=…` boots and dumps a
BMP that matches the Phase 5 golden. Manual checklist recorded in-file: the
boo menu boots the //e; 80-column BASIC works; DHGR demo runs; Apple keys and
sound work.

## Phase 8 — Parity sweep, self-test, ProDOS 80-col, docs (M)

**Goal:** Nothing a real Enhanced //e obviously does that EWM's //e doesn't,
within scope; the machine is documented and shipped.

**Scope:**
- Run the //e ROM **self-test** (Solid-Apple + Ctrl-Reset, or the ROM entry)
  headless far enough to assert it reports RAM/ROM OK — a strong burn-in for
  the MMU and aux routing.
- Boot **ProDOS 2.4.3** (already in `disks/`) on the //e and assert its
  80-column Bitsy Bye / `CAT` renders in 80 columns — an end-to-end aux + 80
  col + clock (Phase already has the slot-1 clock) integration gate.
- Remove/retire the "apple2e returns an error" quirk (#4 in `REWRITE.md`) and
  reconcile any soft-switch warnings (`TOTAL_RECALL_WRITE_WARNINGS.md` — many
  of those "unexpected" ][+ writes are now *implemented* //e switches).
- README: add the //e to "What's emulated" and the run examples; add a
  filled-in parity checklist (switch by switch, mode by mode) to this file.

**Gate:** Full `cargo test` green including all new //e tests; self-test and
ProDOS-80col gates pass; README updated and verified by following it.

---

## Quirks & divergences (record as they are decided)

Seed list; append during implementation, mirroring `REWRITE.md`'s
"Quirks to preserve" / "Documented divergences":

1. **64K //e is a valid config** — before Phase 4 (and selectable after),
   the //e runs with no aux card; aux switches are inert and aux reads fall
   back to main. This is real hardware, not a stub.
2. **Status-switch open bus** — reading `$C011-$C01F` returns the switch
   state in bit 7; the low 7 bits on real hardware carry the last value on
   the video bus (often the current character). EWM returns bit 7 only
   (0 in the low bits) unless a program is found to depend on the open-bus
   value.
3. **RDVBL (`$C019`)** — vertical blank is not cycle-modeled; EWM returns a
   plausible fixed/derived value. Software that busy-waits on VBL for timing
   may run fast, as the ][+ already does with the fake MHz display.
4. **Apple-key mapping** — Open/Solid-Apple map to host modifier keys, not
   the physical //e keycaps, because AppKit reserves some Cmd combinations
   (see the existing Cmd-Esc → Cmd-R note in `REWRITE.md`).
5. **65C02 //e timing deltas** are not modeled (top-level decision), matching
   the rewrite's existing stance.
6. **Character ROM `+1` offset** in the current `chr.rs` is specific to the
   ][+ 2716 dump and is *not* reused for the //e 4K ROM.

## Risks & open questions

- **ROM redistribution.** The Enhanced //e ROMs are the gating dependency for
  every code phase. Confirm the project is comfortable checking them in (it
  already ships `341-00xx` + the char ROM) and record hashes.
- **Internal-vs-slot ROM arbitration** (`$C100-$CFFF`, `$C300`, `$C800`
  expansion) is the subtlest boot-critical piece — the 80-col firmware lives
  in the internal `$C800` space and must appear/disappear per
  SLOTCXROM/SLOTC3ROM. Budget extra care in Phase 2/5; the ProDOS + `PR#3`
  gates will catch mistakes.
- **Aux interleave convention** (aux=even columns) — verify against real ROM
  80-col output before baking golden BMPs.
- **Renderer width migration.** Introducing a 560-wide path risks touching
  shared `scr.rs` code the ][+ golden test depends on. Keep the //e path
  additive; only unify later behind its own gate.
- **Perf of `base_ram_size = 0`.** Routing ZP/stack through a device is
  within budget per the rewrite benches, but re-measure with the //e's
  `mem_bench` if the accelerated (7.16 MHz) mode feels sluggish. Page-pointer
  RAM is the escape hatch (Future work).

## Sequencing notes

- Phases are ordered 0→8. Phases 1 (char ROM) and 2 (64K boot) are the only
  ones with no aux-memory dependency and can proceed as soon as ROMs land.
- Phase 4 (aux + MMU) is the linchpin: Phases 5 and 6 depend on it entirely.
- Phase 7 (windowing) can trail 5/6 or interleave with them if a windowed
  smoke test is wanted earlier; nothing else reorders.
- Every phase keeps the Apple ][+ gates green — that is the regression net.

## Future work (out of scope for this plan)

- Original **NMOS //e** (unenhanced) and the **Apple //c**.
- A **RamWorks**-style aux expansion beyond 128K.
- Cycle-exact 65C02 //e timing; real VBL modeling.
- **Page-pointer RAM** in `ewm-core` (a generic 256-entry read/write page
  table) as a faster, still-generic alternative to device-routed RAM — only
  if profiling demands it.
- Unifying the ][+ (280) and //e (560) renderers into one width-parametric
  path.
- `.woz` images and Disk II write-back (already tracked in `REWRITE.md`).
</content>
</invoke>
