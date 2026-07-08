# Apple //e Enhanced Support — Implementation Plan

A working document for adding the **Enhanced Apple //e** to EWM as a third
machine alongside the Apple 1 / Replica 1 (`one`) and the Apple ][+ (`two`).
Like `REWRITE.md`, this is meant to be re-read at the start of every session
and updated as phases land. **The tree must build and pass all verification
gates (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`) after every phase.**

> **Branch:** All Apple //e Enhanced work happens on the long-lived
> **`claude/apple-iie-enhanced`** integration branch — it stays separate from
> `master` until every phase (and any follow-up polish) is done. Each
> sub-phase is developed on its own branch cut *from* `claude/apple-iie-enhanced`
> and opened as a PR *into* `claude/apple-iie-enhanced` (never into `master`).
> Only when the whole feature is complete does one final PR promote
> `claude/apple-iie-enhanced` → `master`. Do **not** target `master` with any
> individual phase PR.

The work is deliberately sliced into many small, PR-sized phases. The eight
themes below (0–8) are the narrative; each is split into **2–3 lettered
sub-phases** (`2a`, `2b`, `2c`, …). **One sub-phase = one PR.** Every
sub-phase is independently completable, leaves the existing Apple ][+ path
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

Each row is one PR. Sub-phases within a theme are ordered; the letter is the
PR sequence.

| Phase | Description | Size | Status |
|---|---|---|---|
| 0 | Plan, ROM assets, and the //e memory/soft-switch map | S | ROMs in (PR #217); map documented |
| 1a | Enhanced char ROM → **primary** glyph set (pure, unit-tested) | S | Done (with 1b) |
| 1b | Enhanced char ROM → **alternate** set + MouseText | S | Done (with 1a) |
| 2a | Machine skeleton: 65C02 + //e system ROM, runs in ROM | M | Done |
| 2b | Internal `$CX` ROM vs slot-card ROM arbitration | M | Done |
| 2c | //e `$C000-$C01F` soft switches → boots headless to `]` (40 col) | M | Done |
| 3a | ALTCHARSET-aware 40-column text (lower case + MouseText display) | M | Done |
| 3b | //e keyboard: lower case + Open/Solid-Apple keys | S | Done |
| 4a | Aux RAM + RAMRD/RAMWRT routing (`$0200-$BFFF`) | M | Done |
| 4b | ALTZP routing (zero page, stack, language-card aux bank) | M | Done |
| 4c | 80STORE display-page routing + AUXMOVE round-trip | M | Done |
| 5a | 560-wide //e render buffer (40-col/LGR/HGR pixel-doubled) | M | Done |
| 5b | 80-column text (main/aux interleave) + golden | M | Done |
| 6a | DHIRES/AN3/IOUDIS plumbing + double-lo-res (DLGR) | M | Done |
| 6b | Double-hi-res (DHGR) rendering + color | M | Done |
| 7a | `two::main` //e path: `--model`, 560-wide windowing | M | Done (560 windowing landed in 5a) |
| 7b | boo menu entry + CLI dispatch | S | Not started |
| 8a | ROM self-test gate + quirk/doc reconciliation | M | Not started |
| 8b | ProDOS 80-col boot gate + README + parity checklist | M | Not started |

## Ground rules (apply to every phase)

- **All work lands on `claude/apple-iie-enhanced`.** Every sub-phase branches
  from, and PRs back into, the `claude/apple-iie-enhanced` integration branch
  — never `master`. The branch is promoted to `master` in a single final PR
  once the whole feature is done. (See the Branch callout at the top.)
- **The Apple ][+ is frozen.** Every existing gate — `two_boot`, `two_dos`,
  `two_hdd`, `two_clk`, `two_timing`, and the `boot_screen_matches_golden_bmp`
  golden screenshot — must stay green and unchanged. The //e is additive.
- Each sub-phase is one PR-sized unit with the gate commands listed in its
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
| Soft-switch device selection | `Two` holds `io: MachineIo`, an enum `Plus(DeviceHandle<TwoIo>)` \| `E(DeviceHandle<IouE>)` — **not** two `Option` fields. The shared host-facing API (keyboard, display mode/page/altcharset, screen-dirty, buttons, joystick, speaker, debug) is a **`SoftSwitches` trait** implemented by both `TwoIo` and `IouE`; `Two::switches()` / `switches_mut()` are the single place the model `match` lives (returning `&dyn SoftSwitches`), and every accessor delegates. Only `ram`/`aux_ram` keep a small direct match, since the ][+'s RAM lives in `Memory`, not in `TwoIo`. *(Deferred through Phases 2c–4a as per-accessor `match self.io`; the trait was extracted once running the //e made the full surface real — see the "run it" step and the SoftSwitches-cleanup PR.)* | Avoids `if apple2e …` scattering; the dispatch is in one function, and a new host accessor is a one-line delegation. |
| Where banking lives | Auxiliary memory + the MMU/IOU soft switches live in the **`ewm` crate as a `Device`** (working name `Mmu`/`IouE`), the same pattern as `Alc`. The //e's `Memory` is built with **no base-RAM fast path** (`Memory::new(0)`) so *all* RAM (incl. zero page and stack) flows through the aux-aware device. | Zero changes to `ewm-core`. Reuses a proven seam. Per-access `dyn Device` dispatch is far inside the 1 MHz (even 7.16 MHz accelerated) budget — the rewrite benches already proved `ind*`-style dispatch is ~200 ms / 100M ops. |
| Aux/switch state sharing | A **single //e memory-management device owns everything the MMU/IOU arbitrate**: main RAM, aux RAM, and the `$C000-$C01F` soft switches. It is mapped over `$0000-$BFFF`, `$C000-$C01F`, and (ALTZP-aware) shares the language-card region. The Disk II / clock / HDD keep their own `$C0Ex/$C09x/$C0Fx` sub-ranges and shadow it via the newest-first region walk (as `Dsk` already shadows `TwoIo` today). | The real MMU/IOU are the central arbiters; centralizing their state avoids `Rc<RefCell>` (which the project deliberately avoids) and matches the hardware. |
| Renderer resolution | The //e renders into a **560×192** internal buffer: 80-column text and double-hi-res are native 560; 40-column / LGR / HGR pixels are drawn 2× horizontally. The ][+ keeps its **280×192** `Scr` path and golden test untouched. | 80-col and DHGR are inherently double-horizontal-res. A separate //e render path avoids disturbing the ][+ golden BMP. Unifying the two renderers is optional later cleanup. |
| ROM assets | **Present in `rom/` as of PR #217**, same stance as the existing `341-00xx` ][+ ROMs. System ROM is two 8K halves: `Apple IIe CD Enhanced - 342-0304-A - 2764.bin` (`$C000-$DFFF`) + `Apple IIe EF Enhanced - 342-0303-A - 2764.bin` (`$E000-$FFFF`); character generator is `Apple IIe Video - Enhanced - 342-0265-A - 2732.bin` (4K, primary + alternate/MouseText sets). Load via `include_bytes!`. | Consistent with how EWM already ships machine + character ROMs. Filenames/sizes/hashes recorded in Phase 0. |
| Memory size | Default the //e to **128K** (64K main + 64K aux, i.e. the Extended 80-Column Text Card fitted). The 64K "no aux card" config is a real intermediate milestone (the Phase 2 bring-up) but the shipped machine is 128K. | 80-col, DHGR, and virtually all //e software assume the extended card. |
| 65C02 timing | Reuse the current 65C02 instruction timings. The //e-specific cycle deltas (decimal-mode +1, fixed page-cross reads) stay **out of scope**, consistent with the rewrite's existing note that "65C02-specific timing remains out of scope." | Keeps parity with the decision already recorded in `REWRITE.md`; the display/boot gates compare architectural state, not cycles. |

## The //e memory map & soft switches (reference)

This table is the contract the aux/display phases (4a–6b) implement.
Addresses are the standard //e I/O locations. "R" = read triggers/reports,
"W" = write triggers.

### Memory-management soft switches (`$C000-$C00F`, write to set)

| Off | On | Name | Effect | First implemented |
|---|---|---|---|---|
| `$C000` W | `$C001` W | 80STORE | When on, PAGE2 routes text page 1 (`$0400-$07FF`) — and, with HIRES on, hi-res page 1 (`$2000-$3FFF`) — to aux, overriding RAMRD/RAMWRT | 4c |
| `$C002` W | `$C003` W | RAMRD | Reads of `$0200-$BFFF` come from aux (on) or main (off) | 4a |
| `$C004` W | `$C005` W | RAMWRT | Writes to `$0200-$BFFF` go to aux (on) or main (off) | 4a |
| `$C006` W | `$C007` W | SLOTCXROM/INTCXROM | Select peripheral-slot ROM vs internal ROM at `$C100-$CFFF` | 2b |
| `$C008` W | `$C009` W | ALTZP | Zero page, stack (`$0000-$01FF`) and language-card RAM come from aux (on) or main (off) | 4b |
| `$C00A` W | `$C00B` W | SLOTC3ROM | Slot-3 ROM (`$C300`) vs internal 80-col firmware | 2b |
| `$C00C` W | `$C00D` W | 80COL | 80-column video (on) vs 40 (off) | 5b |
| `$C00E` W | `$C00F` W | ALTCHARSET | Alternate character set (MouseText/lower-inverse) vs primary | 3a |

### Status reads (`$C010-$C01F`, read returns switch state in bit 7)

| Addr | Name | Reports | First implemented |
|---|---|---|---|
| `$C010` R | KBDSTRB / AKD | Clears the keyboard strobe; bit 7 = any-key-down | 2c |
| `$C011` R | RDLCBNK2 | Language-card bank 2 selected | 2c |
| `$C012` R | RDLCRAM | Language-card RAM read-enabled | 2c |
| `$C013` R | RDRAMRD | RAMRD state | 4a |
| `$C014` R | RDRAMWRT | RAMWRT state | 4a |
| `$C015` R | RDCXROM | INTCXROM state | 2b |
| `$C016` R | RDALTZP | ALTZP state | 4b |
| `$C017` R | RDC3ROM | SLOTC3ROM state | 2b |
| `$C018` R | RD80STORE | 80STORE state | 4c |
| `$C019` R | RDVBL | Vertical-blank (see Quirks) | 2c |
| `$C01A` R | RDTEXT | TEXT mode | 2c |
| `$C01B` R | RDMIXED | MIXED mode | 2c |
| `$C01C` R | RDPAGE2 | PAGE2 state | 2c |
| `$C01D` R | RDHIRES | HIRES mode | 2c |
| `$C01E` R | RDALTCHAR | ALTCHARSET state | 3a |
| `$C01F` R | RD80COL | 80COL state | 5b |

### Display & misc (mostly reuse the existing ][+ `TwoIo` handlers)

`$C050-$C057` TEXT/MIXED/PAGE2/HIRES (already handled), `$C05E`/`$C05F`
**DHIRES** on/off (double-res enable, interacts with AN3), `$C07E`/`$C07F`
**IOUDIS**/RDIOUDIS, `$C061-$C063` Open-Apple / Solid-Apple / Shift buttons
(already read as buttons), `$C080-$C08F` language card (folded into `IouE`
with ALTZP-selected aux banks — 4b).

## Current architecture — what each phase touches

- `ewm-core/src/{cpu,mem,ins,fmt}.rs` — generic kernel. **Untouched** by this
  work (65C02 already present). If a tiny accessor is unavoidable (e.g. to let
  the renderer see aux RAM) it must remain Apple-agnostic.
- `ewm/src/two.rs` — the machine + SDL loop. Gains a `model` field, //e ROM
  loading, the `Mmu` device wiring, and //e branches in the frame loop.
- `ewm/src/alc.rs` — the ][+ language card. **Left untouched** (a pure ][+
  device); the //e language card is folded into `IouE` in `two.rs`, where it
  can share the ALTZP state (4b).
- `ewm/src/chr.rs` — character generator. Gains the enhanced 4K char ROM
  decode (primary + alternate sets, lower case, MouseText).
- `ewm/src/scr.rs` — renderer. Gains a 560-wide //e path: 80-col text, DLGR,
  DHGR, ALTCHARSET selection.
- `ewm/src/boo.rs`, `ewm/src/main.rs` — menu + CLI, gain the //e entry.
- Tests: new `ewm/tests/two_e_*.rs`; new `rom/` and possibly `disks/` assets.

---

## Phase 0 — Plan, ROM assets, and the memory/soft-switch map (S)

**Goal:** This document exists, the Enhanced //e ROMs are checked in, and the
machine map above is verified. No behavior change.

**Status:** ROMs landed in `rom/` via **PR #217** ("Adding Apple IIe Enhanced
ROMs"). The human dependency below is resolved. Recorded assets (SHA-256
prefixes for verification):

| File | Size | Maps to | sha256 (first 16) |
|---|---|---|---|
| `Apple IIe CD Enhanced - 342-0304-A - 2764.bin` | 8192 | `$C000-$DFFF` | `f5255e59b335e738` |
| `Apple IIe EF Enhanced - 342-0303-A - 2764.bin` | 8192 | `$E000-$FFFF` | `3ccfd0bf9f2c87b4` |
| `Apple IIe Video - Enhanced - 342-0265-A - 2732.bin` | 4096 | char gen (2 sets) | `52c3b87900ac939f` |
| `Apple IIe Keyboard - 341-0150-A - 2716.bin` | 2048 | keyboard encoder | `dabc2f4a2804e92e` |
| `Apple IIe Keyboard - 342-0132-B - 2716.bin` | 2048 | keyboard encoder | `68198ae95923926b` |
| `Apple IIe Keyboard - 342-0132-C - 2716.bin` | 2048 | keyboard encoder | `fbb9620e01f4f728` |
| `Apple IIe Keyboard - 342-0132-D - 2716.bin` | 2048 | keyboard encoder | `a1989da84ea4381d` |

The two 8K system halves concatenate to the 16K `$C000-$FFFF` image (the
`$C000-$CFFF` quarter is the internal I/O firmware / peripheral-slot ROM;
`$D000-$FFFF` is Monitor + AppleSoft, banked by the language card). The 4K
video ROM holds the primary and alternate (MouseText) glyph sets, 8
bytes/glyph. The keyboard-encoder ROMs are informational — EWM synthesizes
keystrokes directly (see the keyboard decision in Phase 3b), so they are not
`include_bytes!`-loaded unless we later choose to model the encoder.

**Scope (remaining):**
- Confirm `TwoType::Apple2E` is the intended Enhanced-//e handle; leave
  `Two::new(TwoType::Apple2E)` returning its current error until Phase 2a.

**Gate:** Tree builds, all existing tests green, ROMs present with recorded
hashes. `git grep -n Apple2E` shows the plumbing points a human will touch.

---

## Phase 1 — Enhanced character ROM → glyph tables

Pure, unit-tested decoding of the 4K enhanced video ROM into glyph bitmaps —
the same "decode is core, textures are frontend" split the ][+ char ROM uses.
No machine, no SDL. Split by character set.

> **Landed (1a + 1b together).** Both sets decode in `ewm/src/chr.rs` via a
> new `ChrE` / `CharSet { Primary, Alternate }`, leaving the ][+ `Chr`
> untouched. Findings recorded from decoding `342-0265-A`:
>
> - **No `+1` offset.** Rows are `rom[idx*8 + y]` (the ][+ `+1` is specific to
>   that 2716 dump, as suspected).
> - **Bit order is reversed vs the ][+** *(corrected when the render landed —
>   see the `--model` step)*: the //e ROM stores the **leftmost** pixel in
>   **bit 0**, so bits are scanned low-to-high (bit 0 → bit 6). The original
>   1a/1b decode scanned high-to-low like the ][+, which mirrored every glyph
>   horizontally; it slipped through because Phase 1's template match only
>   compared *symmetric* letters (A, H, I, M, O, T, U, …) against the ][+ set.
> - **Only the first 2K is used.** It holds the whole repertoire: UC/sym
>   (`$00-$3F`), **MouseText** (`$40-$5F` — the checkerboard sits at `$56`),
>   lower case (`$60-$7F`). The second 2K is not needed; inverse forms are
>   synthesized by XOR exactly as the ][+ decode does.
> - **Correction to the plan's wording:** lower case and MouseText live in the
>   **alternate** set, *not* the primary set. The primary set is Apple ][
>   compatible — upper case and symbols only (all codes map to ROM `$00-$3F`,
>   top bit = normal/inverse). So the "1a decodes lower case" line below is
>   folded into 1b's alternate set. The display-code translation implemented is
>   the standard //e one (e.g. alt `$E1`→ROM `$61` lower 'a'; alt `$41`→ROM
>   `$41` MouseText; alt `$C1`==primary `$C1`=='A').
> - Both sets are returned as `&Glyph` (every //e code maps to a glyph, so no
>   `Option`). Flashing (`$40-$7F` primary) is rendered in its inverse phase,
>   matching the existing ][+ decode.

### Phase 1a — Primary character set (S)

**Goal:** Decode the primary glyph set (upper/lower case + inverse +
flashing) into `[Option<Glyph>; 256]`.

**Scope:**
- Re-derive the //e ROM byte layout: the ][+ `chr.rs` `rom[c*8 + y + 1]`
  one-byte offset is specific to that 2716 dump and almost certainly does
  *not* apply to `342-0265-A`.
- Add behind a `Chr::new_iie_primary()` (or a `CharSet` enum) without
  disturbing the existing ][+ `Chr`.

**Gate:** Unit tests — 'A' (upper), 'a' (lower, which the ][+ set cannot
render), and an inverse glyph each decode to their expected bitmaps.

### Phase 1b — Alternate set + MouseText (S)

**Goal:** Decode the alternate glyph set, including MouseText.

**Scope:**
- Decode the alternate half: upper/lower + inverse + **MouseText** at codes
  `$40-$5F`. The alternate set replaces the primary's flashing range with
  MouseText — the //e's real behavior.
- Expose both sets so the renderer can pick per ALTCHARSET (Phase 3a).

**Gate:** Unit tests — a MouseText glyph (e.g. the "open-apple") and an
alternate-set lower-case glyph decode as expected.

---

## Phase 2 — 64K //e bring-up (boots headless to `]`)

Stand up a real 64K "no extended card" //e that boots to 40-column
AppleSoft. Aux memory arrives in Phase 4; the aux switches here are inert
(main answers). Everything below is `Apple2E`-only — the ][+ path is untouched.

### Phase 2a — Machine skeleton: 65C02 + //e system ROM (M)

**Goal:** `Two::new(TwoType::Apple2E)` constructs a 65C02 //e that fetches
and executes the //e reset vector — it runs *in ROM*, even though it cannot
finish booting until 2b/2c.

> **Landed.** `Two::new` dispatches to `new_2plus()` / `new_2e()`; the //e
> builds a `Model::M65C02` with the banked `$D000-$FFFF` ROM
> (`ROM_IIE_CD[$1000..]` + `ROM_IIE_EF`, 12K) via the reused `Alc`, a stub
> `IouE` over `$C000-$C07F`, and the ][+ slot layout (Disk II `$C600`, clock
> `$C100`); 64K base RAM. `io` became the `MachineIo` enum (see the decision
> table); `apple2_and_apple2e_are_unsupported` narrowed to
> `apple2_is_unsupported`.
>
> Findings from the running skeleton (`ewm/tests/two_e_skeleton.rs`):
> - Reset vector `$FFFC` = **`$FA62`** (monitor RESET, first opcode `$D8` CLD).
> - The 65C02 executes the monitor cold start in banked ROM (`$E000-$FFFF`),
>   then wanders into the internal **`$C300`** 80-column firmware and gets
>   stuck spinning around `$C3FA` — because internal `$CX` ROM is not mapped
>   yet. That is exactly the 2a boundary and motivates 2b. The
>   deliberately-deferred internal `$C100-$CFFF` ROM (kept as slot ROMs here)
>   is what 2b arbitrates.

**Scope:**
- `Two::new` branches on `model`: for `Apple2E`, build the CPU with
  `Model::M65C02` and load the //e system ROM (`$D000-$FFFF` via the reused
  `Alc`; `$C000-$CFFF` as internal ROM for now, refined in 2b).
- A stub `Mmu`/`IouE` device over `$C000-$C01F` that stores switch state and
  returns benign values (no aux yet). Reuse the ][+ `$C050-$C057` display
  switches.
- Keep the ][+ `TwoIo` path untouched; the //e uses the new device. Slot 6
  Disk II, slot 7 HDD, slot 1 clock attach exactly as today.

**Key decisions:** 64K, aux switches inert. This is the PR where `Apple2E`
stops erroring, so `two_boot`'s `apple2_and_apple2e_are_unsupported` test is
narrowed to `Apple2` only.

**Gate:** `ewm/tests/two_e_skeleton.rs` — construct `Apple2E`, reset, step a
fixed cycle budget without panicking, and assert the PC has entered the //e
ROM's cold-start path (a known early routine / ROM address range).

### Phase 2b — Internal `$CX` ROM vs slot-card ROM arbitration (M)

**Goal:** `$C100-$CFFF` correctly switches between the internal firmware and
the peripheral-slot ROMs.

> **Landed.** The arbitration lives **inside `IouE`** (per the "single device
> owns everything" decision): the static region walk can't express a
> runtime-switchable source, so `IouE` is mapped over `$C000-$C07F` *and*
> `$C100-$CFFF`, holds the internal firmware (`ROM_IIE_CD[$100..$1000]`) plus
> the peripheral slot ROM images (clock/disk, and hdd via a model-aware
> `attach_hdd`), and routes each read. The slot ROMs moved out of `new_2e`'s
> `add_rom` calls into `IouE::set_slot_rom`; the Disk II / clock *I/O* devices
> stay separate.
>
> Switches: `$C006/$C007` (INTCXROM), `$C00A/$C00B` (SLOTC3ROM) write-to-set;
> `$C015/$C017` report state in bit 7. Read routing: INTCXROM → internal
> everywhere; else `$C300` = internal 80-col firmware (or open bus when
> SLOTC3ROM selects the absent slot-3 card), `$Cn00` = that slot's card ROM or
> open bus. The `$C800-$CFFF` expansion latch is a single `c800_internal` flag
> (only the internal slot-3 firmware has a `$C800` image here) — set on a
> `$C3xx` access, cleared by a `$CFFF` access, forced on under INTCXROM;
> per-slot expansion ROM is out of scope (no EWM card has one).
>
> Findings: the internal `$C300` firmware carries the Pascal-1.1 signature
> (`$C305=$38`, `$C307=$18`); internal `$C600=$8D` vs disk slot `$A2`, internal
> `$C100=$4C` vs clock slot `$08` — all directly asserted. **This unsticks the
> 2a `$C3FA` spin:** the //e now runs the monitor + 80-col firmware and reaches
> the slot-6 Disk II boot ROM (`$C65E`), where it waits for a disk (boot to `]`
> is 2c). Gate: `ewm/tests/two_e_cxrom.rs` (5 tests).

**Scope:**
- Implement INTCXROM/SLOTCXROM (`$C006`/`$C007`), SLOTC3ROM
  (`$C00A`/`$C00B`), and the `$C800-$CFFF` shared expansion-ROM space (with
  the `$CFFF` expansion-ROM reset). The internal 80-column firmware lives
  here.
- Compose with the existing slot ROMs (Disk II `$C600`, HDD `$C700`, clock
  `$C100`) via the newest-first region walk; `RDCXROM`/`RDC3ROM` report state.

**Key decisions:** This is the subtlest boot-critical piece (see Risks). Get
the precedence right now so 2c and the 80-col firmware (5b) work.

**Gate:** Unit tests — under each (INTCXROM, SLOTC3ROM) combination, assert
reads at `$C300` / `$C800` / `$CFFF` come from the expected ROM.

### Phase 2c — //e soft switches → boots to `]` (M)

**Goal:** The //e boots DOS 3.3 to the AppleSoft `]` prompt in 40 columns,
fully headless.

> **Landed — boots DOS 3.3 and evaluates `PRINT 2+2` → `4`.** `IouE` now tracks
> the `$C000-$C00F` memory switches (80STORE/RAMRD/RAMWRT/ALTZP — **state only**,
> the aux routing is Phase 4) and the display switches (`$C050-$C057`
> TEXT/MIXED/PAGE2/HIRES, `$C00C-$C00F` 80COL/ALTCHARSET), and answers all
> `$C010-$C01F` status reads in bit 7. RDVBL (`$C019`) is derived from the cycle
> counter (not cycle-modelled — quirk #3). RDLCBNK2/RDLCRAM (`$C011`/`$C012`)
> are answered by the language card, which now shadows those two addresses so
> it reports its own state (matched before `Alc::bank_read`, whose `$D000`
> offset would otherwise underflow). `key()`/`key_register()` became
> model-aware (both `TwoIo` and `IouE` own a keyboard latch) — the `SoftSwitches`
> trait stays deferred; keyboard was the only shared host API 2c needed.
>
> **The one real bug this surfaced:** the //e enhanced firmware clears the
> keyboard strobe with a **write** (`STA $C010`), not a read like the ][+
> monitor. `IouE` cleared KBDSTRB only on read, so every typed key re-latched
> forever (the screen filled with one character). Clearing on *any* `$C010`
> access fixed it — and it was the whole blocker: the //e already reached a live
> keyboard-wait loop (in the internal `$C27D` firmware, not the monitor
> `$FD1D`) but couldn't consume input. The "hang loading Integer BASIC" was a
> red herring — DOS boots fully; it was the strobe.
>
> Gate: `ewm/tests/two_e_boot.rs` (4 tests) — the DOS 3.3 boot + `PRINT 2+2`,
> plus memory/display switch round-trips and the strobe clear-on-read/write.
> Deferred as planned: speaker (`$C030`) → 7; Open/Solid-Apple buttons
> (`$C061`/`$C062`) → 3b; DHIRES (`$C05E`/`$C05F`) → 6a; aux routing → 4. The
> 80-column display (`PR#3`) still misrenders until 5b — 2c only tracks 80COL.

**Scope:**
- Flesh out the `$C000-$C00F` write-to-set switches (state only; aux still
  absent) and the `$C010-$C01F` read-status switches (state in bit 7; `$C010`
  clears the strobe and reports AKD; RDLCBNK2/RDLCRAM read the `Alc` state).
- Whatever else the ROM cold-start touches to reach BASIC.

**Key decisions:** `PR#3` (80-col) is expected to fail until Phase 5b.

**Gate:** `ewm/tests/two_e_boot.rs` — insert `DOS33-SystemMaster.dsk`, reset,
step until `text_screen()` shows `]`, type `PRINT 2+2`, assert `4`. (DOS 3.3
runs on a //e.)

---

## Phase 3 — //e 40-column display & keyboard

Two independent halves — display and input — that make the windowed //e
usable in 40 columns.

### Phase 3a — ALTCHARSET-aware 40-column text (M)

**Goal:** 40-column //e text renders lower case and, with ALTCHARSET on,
MouseText.

> **Landed (with 3b) — as the decode layer, not a throwaway pixel draw.** The
> 3a gate is headless (glyph selection + text scrape) and the real 560-wide
> pixel render is Phase 5a, so a 280 draw here would be discarded. Instead 3a
> delivers the reusable pieces the 5a renderer consumes: `ChrE::glyph(altcharset,
> code)` selects alternate vs primary; `Two::alt_charset()` and a model-aware
> `Two::screen_page()` expose the //e display state; and `text_screen()` is now
> //e-aware via `screen_code_to_char_e` (lower case preserved; ALTCHARSET drives
> the `$60-$7F` inverse-lower-case vs flashing-symbol range). Safe for the 2c
> boot gate — DOS 3.3 runs ALTCHARSET off (primary set), so the scrape is
> unchanged there. Gate: `ewm/tests/two_e_text.rs` (3 tests: MouseText glyph
> selection, lower-case scrape, the ALTCHARSET `$61` distinction).

**Scope:**
- A //e 40-column text render path that selects the primary vs alternate
  glyph set (Phase 1a/1b) from ALTCHARSET (`$C01E`). Still 280-equivalent
  (a temporary 40-col draw, folded into the 560 buffer once 5a lands).
- Consume the ALTCHARSET / PAGE2 state the `Mmu` already tracks.

**Gate:** Headless — with ALTCHARSET on, a `$40-$5F` byte poked into the text
page scrapes to its MouseText glyph; a //e-aware `text_screen()` preserves
lower case for a poked lower-case string.

### Phase 3b — //e keyboard: lower case + Apple keys (S)

**Goal:** Lower-case input and the Open/Solid-Apple keys.

> **Landed (with 3a).** The SDL `TextInput` handler no longer upper-cases for
> the //e (`two.model() == Apple2E` → pass the byte through; the ][+ still
> upper-cases, as its ROM has no lower case). `IouE` gained the game-I/O
> `buttons` array read at `$C061`/`$C062`/`$C063` (Open-Apple / Solid-Apple /
> shift-mod, bit 7 = pressed), and `set_button()` became model-aware — the
> existing Alt-1/Alt-2 and gamepad mappings now drive the //e buttons too.
> Gate: `ewm/tests/two_e_keyboard.rs` (2 tests: Open/Solid-Apple read at
> `$C061`/`$C062`; and an end-to-end lower-case echo — boot DOS 3.3, type
> `PRINT "hello"`, get lower-case `hello`, proving the latch takes lower case
> verbatim). The SDL upper-casing removal itself is a frontend change (manual);
> the lower-case echo test covers the machine + scrape path headlessly.

**Scope:**
- Stop force-upper-casing input for the //e (the ][+ path upper-cases in
  `two.rs`); pass lower case through.
- Map host modifiers to **Open-Apple** (button 0, `$C061`) and
  **Solid-Apple** (button 1, `$C062`) — needed for the ROM self-test and many
  games.

**Key decisions:** Upper-casing stays for the ][+ (its ROM has no lower
case). Apple-key mapping is a documented divergence (macOS reserves some Cmd
combos, per the existing Cmd-Esc → Cmd-R note).

**Gate:** Headless — a typed lower-case line scrapes as lower case;
Open-Apple reads pressed at `$C061`. Manual (recorded in-file): lower case
echoes; a MouseText demo renders.

---

## Phase 4 — Auxiliary memory + MMU routing

The architectural heart of //e support: a second 64K aux bank and the full
routing truth table, built up one rule at a time. All three sub-phases share
the single `Mmu` device and the `Memory::new(0)` construction.

### Phase 4a — Aux RAM + RAMRD/RAMWRT (`$0200-$BFFF`) (M)

**Goal:** A second 64K aux bank with the main-body read/write routing.

> **Landed.** `IouE` now owns two 48K banks (`main`, `aux`) for `$0000-$BFFF`
> and is mapped over that range, so `new_2e` builds `Memory::new(0)` — no
> base-RAM fast path, all low memory flows through the device. Reads of
> `$0200-$BFFF` follow RAMRD, writes follow RAMWRT; `$0000-$01FF` (ZP + stack)
> stays in main until ALTZP (4b). `Two::ram()` is model-aware (//e → main
> bank) and a new `aux_ram()` exposes the aux bank; the renderer and
> `text_screen` read `ram()` unchanged, so they keep reading the display page
> from main (80STORE routing is 4c).
>
> Regression-safe because RAMRD/RAMWRT default off — the //e runs entirely in
> main, byte-identical to the old base RAM: DOS still boots and `PRINT 2+2` →
> `4`, and **the `two-e-40col.bmp` golden still matches** (content unchanged,
> storage moved). **Perf:** the `$0000-$BFFF` map is added *last* in `new_2e`
> so the newest-first region walk checks RAM first (one comparison on the hot
> zero-page/stack path); the full suite time is unchanged in practice. Gate:
> `ewm/tests/two_e_aux.rs` (4 tests — the RAMRD×RAMWRT truth table, bank
> inspection via `ram()`/`aux_ram()`, ZP/stack staying in main, and
> RDRAMRD/RDRAMWRT state).

**Scope:**
- The `Mmu` owns main + aux banks; build the //e `Memory` with
  `Memory::new(0)` so **all** RAM flows through it.
- `$0200-$BFFF`: reads follow RAMRD (`$C002`/`$C003`), writes follow RAMWRT
  (`$C004`/`$C005`). `$0000-$01FF` and the LC region stay main-only for now.
- `Two` exposes `main_ram()` / `aux_ram()` accessors for the renderers (the
  "renderer reads `&Two` between step batches" discipline the ][+ uses).

**Gate:** `ewm/tests/two_e_aux.rs` — a RAMRD×RAMWRT truth table over
`$0200-$BFFF` (sentinel lands in and reads back from the correct bank);
`RDRAMRD`/`RDRAMWRT` reflect state.

### Phase 4b — ALTZP: zero page, stack, language-card aux (M)

**Goal:** ALTZP banks the zero page, stack, and language-card RAM.

**Scope:**
- `$0000-$01FF` (ZP + stack) follow ALTZP (`$C008`/`$C009`). The CPU's
  push/pull already go through `mem`, so this "just works" once the device
  routes it.
- Extend `Alc` (or fold LC into `Mmu`) so `$D000-$FFFF` card RAM has main +
  aux copies selected by ALTZP.

**Gate:** ALTZP truth table (ZP + stack writes land in the selected bank;
`RDALTZP` reflects state); an LC-aux test extending the existing
language-card tests.

> **Landed.** ALTZP (`$C008`/`$C009`) now routes both the zero page + stack
> (`$0000-$01FF`) and the language-card RAM. The `Alc` peripheral card is left
> **byte-for-byte a ][+ device** — no //e logic — and the //e language card
> was **folded into `IouE`** instead, co-locating it with the ALTZP state it
> depends on. `IouE` owns `lc_d1`/`lc_d2`/`lc_e` as `[main, aux]` bank pairs
> plus the shadowed ROM, and reproduces the card's two-reads-to-write-enable
> handshake; `read_ram`/`write_ram` pick main vs aux by ALTZP below `$0200`
> and by RAMRD/RAMWRT above it. `new_2e` drops the `Alc` device and maps
> `IouE` over `$C080-$C08F` and `$D000-$FFFF` (mapped last, so the region walk
> reaches it first). `RDLCBNK2`/`RDLCRAM` (`$C011`/`$C012`) are now answered by
> `IouE`. Gate: `ewm/tests/two_e_altzp.rs`. The ][+ `language_card_*` tests
> stay green and the //e still boots DOS 3.3.

### Phase 4c — 80STORE display-page routing + AUXMOVE (M)

**Goal:** The 80STORE/PAGE2(+HIRES) display-page override, verified
end-to-end.

**Scope:**
- 80STORE (`$C000`/`$C001`) on: PAGE2 routes text page 1 (`$0400-$07FF`) to
  aux regardless of RAMRD/RAMWRT; with HIRES on, hi-res page 1
  (`$2000-$3FFF`) too. This override sits *above* RAMRD/RAMWRT — order
  matters.

**Gate:** 80STORE truth table; a 65C02 program using the ROM `AUXMOVE`
(`$C311`) / `XFER` primitives round-trips a buffer main↔aux; `RD80STORE`
reflects state.

> **Landed.** The override is one helper, `IouE::store80_aux`, applied in both
> `read_ram` and `write_ram`: when 80STORE is on it returns `Some(page2)` for
> text page 1 (`$0400-$07FF`) and — only when HIRES is on — hi-res page 1
> (`$2000-$3FFF`), else `None` to fall through to RAMRD/RAMWRT. It sits *above*
> RAMRD/RAMWRT and, unlike them, uses the same PAGE2 selector for reads and
> writes; ZP/stack still follow ALTZP. `RD80STORE` (`$C018`) was already wired
> in 2c. Gates: `ewm/tests/two_e_80store.rs` (the truth table: page-1 text and
> HIRES-gated hi-res follow PAGE2, page 2 / `$4000` / ordinary RAM keep
> following RAMRD/RAMWRT, and PAGE2 does not route memory when 80STORE is off)
> and `ewm/tests/two_e_auxmove.rs` (a hand-assembled driver round-trips a buffer
> main↔aux through the real Monitor `AUXMOVE` at `$C311`, INTCXROM on). This
> completes the **4a→4b→4c** aux linchpin that 5b and Phase 6 depend on. The
> ][+ path and both goldens are unchanged (80STORE stays off during DOS boot,
> so the override is inert).

---

## Phase 5 — 80-column text display

### Phase 5a — 560-wide //e render buffer (M)

**Goal:** Introduce the //e 560×192 render path at visual parity with the
40-column output (pixel-doubled) — no 80 columns yet.

**Scope:**
- A //e renderer (a new `scr` path or `ScrE`) producing 560×192; 40-column
  text / LGR / HGR draw 2× horizontally. The ][+ 280 path and its golden BMP
  are untouched. ALTCHARSET selection from 3a carries over.

**Gate:** A 560-wide golden BMP of the 40-column boot screen (pixel-doubled)
matches (`ewm/golden/two-e-40col.bmp`, via the `--screenshot` path).

> **Landed.** Kept deliberately additive per the risk note: the shared `Scr`
> code still renders the //e's 40-column content into the 280-wide `pixels`
> (so nothing the ][+ golden depends on moved), and `update()` then
> pixel-doubles it into a new 560-wide `wide` buffer when the model is the //e.
> `Scr::frame(model)` / `scr::frame_width(model)` return the right buffer/width
> (560 //e, 280 ][+); the SDL loop, `--screenshot`, and the golden all go
> through them. **Window size is model-independent** — the 560 texture is
> nearest-stretched into the same on-screen rect, so //e pixels are half-width
> (real-hardware behavior) and the status bar / tty overlay / window sizing
> needed no changes. Gate: `iie_boot_screen_matches_golden_bmp` now renders at
> 560 and `ewm/golden/two-e-40col.bmp` was regenerated (verified: every
> column-pair is identical, i.e. a perfect horizontal double of the 280 render).
> This also finishes **7a**'s deferred "560-wide windowing". 5b will branch the
> *text* path on 80COL (native 560 for 80-col, doubled for 40-col) inside this
> same `wide` buffer.

### Phase 5b — 80-column text (main/aux interleave) (M)

**Goal:** `PR#3` / 80COL produces real 80-column text, verified headless.

**Scope:**
- 80-column text reads interleaved memory: aux = even columns (0,2,4,…),
  main = odd columns, 7 px each → 560. Driven by 80COL (`$C00C`/`$C00D`) plus
  80STORE/PAGE2 (Phase 4c routing already places the aux half). `RD80COL`
  reports state.
- A `text_screen_80()` (24×80) scrape reading both banks — the headless
  workhorse for this and later gates.

**Key decisions:** Verify the aux-even / main-odd convention against the ROM's
own 80-col output before baking goldens.

**Gate:** `ewm/tests/two_e_80col.rs` — enable 80 columns (via `PR#3` from
AppleSoft), print a known string, assert `text_screen_80()` shows it across 80
columns; a checked-in golden 560-wide BMP.

> **Landed.** The aux-even / main-odd convention was **verified against the
> real ROM** first: a headless `PR#3` + `PRINT "ABCDEFGH…"` showed `aux =
> [C1 C3 C5 …]` (A, C, E — even columns) and `main = [C2 C4 C6 …]` (B, D, F —
> odd), exactly the assumed interleave. `two.col80()` (a new `SoftSwitches`
> accessor) and `two.text_screen_80()` (the 24×80 scrape) landed in `two.rs`;
> `Scr::render_txt_screen_80` draws 80 × 7 px **natively into the 560-wide
> `wide` buffer** (no doubling), and `update()` takes that path when the //e is
> in text mode with 80COL on — every other mode keeps the 5a doubled path.
> Gate `ewm/tests/two_e_80col.rs`: the interleave scrape, a direct
> aux-even/main-odd bank check, the `PR#3` firmware path (asserts 80COL +
> ALTCHARSET on and a mixed-case string across 80 columns), and a native-560
> golden (`ewm/golden/two-e-80col.bmp`, confirmed *not* pixel-doubled). `PR#3`
> also turns on ALTCHARSET, which enables **MouseText and inverse lower case**
> (the `$40-$7F` range). *(Correction: normal lower case is **not** gated on
> ALTCHARSET — the primary set shows lower case at `$E0-$FF` too. An earlier
> note here wrongly claimed a bare 40-column //e can't show lower case; that was
> a `primary_index` bug, fixed separately — the "Apple //e" cold-boot banner
> made it visible.)* The ][+ path and the 5a 40-column golden are unchanged.

---

## Phase 6 — Double-res graphics

### Phase 6a — DHIRES/AN3/IOUDIS plumbing + double-lo-res (M)

**Goal:** The double-res control path, plus double-lo-res.

**Scope:**
- DHIRES (`$C05E`/`$C05F`), AN3, and IOUDIS (`$C07E`/`$C07F`) precedence:
  IOUDIS off exposes DHIRES at `$C05E`/`$C05F`; on, those revert to AN3
  control. Store state; the RD switch reflects it.
- **DLGR**: double-width lo-res, aux/main interleaved, reusing the LGR color
  table already in `scr.rs`.

**Gate:** A DLGR smoke render matches a golden; the switch-state reads are
correct.

> **Landed.** The precedence was confirmed with the owner against the //e Tech
> Ref (the scope note above had it backwards): **IOUDIS is the gatekeeper and
> resets *on***, so `$C05E`/`$C05F` are the DHIRES switch out of reset (`$C05E`
> on, `$C05F` off, on any read or write); `CLRIOUDIS` (`$C07F`) hands those
> addresses to annunciator 3 instead. `IouE` gains `ioudis`/`dhires`/`an3`;
> `$C07E`/`$C07F` write IOUDIS and read `RDIOUDIS`/`RDDHIRES` (bit 7).
> `render_dlgr_screen` draws **80-column lo-res natively into the 560-wide
> `wide` buffer** — aux even / main odd (the 5b interleave), each a 7 px LGR
> block reusing the existing color table; `update()` takes it when the //e is in
> lo-res graphics with DHIRES + 80COL (mixed mode renders 80-col text in the
> bottom four rows). Gate `ewm/tests/two_e_dlgr.rs`: the IOUDIS/DHIRES/AN3
> truth table + a native-560 golden (`two-e-dlgr.bmp`, 80 color bars, confirmed
> not doubled). *Deferred to 6b:* the **AN3 falling-edge latching** the owner
> flagged (a documented drift point) — level-based DHIRES is enough for DLGR and
> matches the no-cycle-timing stance (quirk #5). The ][+ path is untouched.

### Phase 6b — Double-hi-res (DHGR) (M)

**Goal:** DHGR renders in monochrome and color.

**Scope:**
- Aux (even 7-px groups) + main (odd) interleave → 560; the 4-bit-per-pixel
  //e color interpretation, a fresh implementation rather than the ][+
  single-hi-res fringing code. Keep a deterministic monochrome 560-wide path
  for the golden test.

**Gate:** `ewm/tests/two_e_dhgr.rs` — a known DHGR bit pattern in main + aux
renders to a golden 560-wide BMP.

> **Landed.** `Scr::render_dhgr_screen` draws a **fresh** 560-wide path into
> `wide` (the ][+ single-hi-res fringing code is untouched): hi-res page 1 in
> both banks, aux even 7-px groups / main odd (the verified 5b interleave), low
> 7 bits per byte with bit 0 leftmost (bit 7 ignored). Monochrome is one pixel
> per bit (green); `update()` takes the path when the //e is in hi-res graphics
> with DHIRES + 80COL (mixed → 80-col text in the bottom four rows). Gate
> `ewm/tests/two_e_dhgr.rs`: aux-is-leftmost-group + bit-0-leftmost interleave
> checks, a colour-cell → palette check, and a native-560 mono golden
> (`two-e-dhgr.bmp`, 40 vertical 7-px stripes). Rendered and eyeballed: mono
> stripes crisp, colour path yields the full 16-colour palette.
>
> **Colour convention (revisit candidate):** the colour path groups the bit
> stream into **aligned 4-bit cells** (leftmost bit = LSB) selecting the 16
> lo-res colours, drawn 4 px wide — the simple, deterministic choice the owner
> approved as a starting point. We may switch to a **sliding 4-bit window**
> (closer to NTSC fringing) after reviewing it against a known DHGR image. The
> mono golden is unaffected by that choice.
>
> **Deferred (with the 6a AN3 note):** the **AN3 falling-edge → double-res
> latch** (IOUDIS-off enable path) — 6b gates DHGR purely on the DHIRES level
> state. To land in a later polish PR / 8a.

---

## Phase 7 — Frontend, menu & CLI

### Phase 7a — `two::main` //e path + windowing (M)

**Goal:** `ewm two --model 2e` runs the //e windowed.

> **Partially landed (intermediate "run it" step).** `ewm two --model 2e`
> boots the //e in a window at **280×192** (title "EWM v0.1 / Apple //e"),
> reusing the existing frame loop and `Scr`: 40-column text (lower case +
> MouseText), lo-res, and hi-res all render — they're 280-equivalent. The
> **560-wide** window + 80-column/DHGR rendering are still Phase 5a/5b/6.
> Wiring this exercised every host-facing accessor, so `screen_mode` /
> `screen_graphics_mode` / `screen_graphics_style` / `screen_dirty` /
> `drain_speaker_toggles` / `set_joystick` all became model-aware (and `IouE`
> gained `screen_dirty` + `speaker_toggles`); the old `io()`/`io_mut()` panic
> accessors are now gone (every accessor is a `match self.io`). **That is the
> full trigger for the deferred `SoftSwitches` trait** — the branching is now
> substantial, so the trait is the natural next cleanup. Gate: a headless //e
> 280 golden (`ewm/golden/two-e-40col.bmp`) rendered through the model-aware
> `Scr`. **This step also fixed a Phase 1 glyph-mirroring bug** (see the Phase 1
> note): the render made the reversed //e ROM bit order visible.

**Key decisions:** Reuse the existing frame loop; branch only on render width
and model. A `--model` flag mirrors `one --model apple1|replica1` and is less
churn than a new subcommand.

**Gate:** Automated — `ewm two --model 2e --screenshot=…` boots and dumps a
BMP matching the Phase 5b golden.

### Phase 7b — boo menu entry + CLI dispatch (S)

**Goal:** The bootloader and top-level CLI expose the //e.

**Scope:**
- boo menu option **4** "APPLE //e — 65C02 / 128K / ENHANCED" returning
  `BooChoice::BootApple2E`; `main.rs` dispatch + updated `usage()`.
- Optional command-palette "40/80 column" toggle if convenient.

**Gate:** `ewm` (no args) → menu → boots the //e. Manual checklist recorded
in-file: 80-column BASIC works; a DHGR demo runs; Apple keys and sound work.

---

## Phase 8 — Parity & polish

### Phase 8a — Self-test gate + quirk/doc reconciliation (M)

**Goal:** The //e ROM self-test passes headless; stale ][+ notes are
reconciled.

**Scope:**
- Drive the //e ROM **self-test** (Solid-Apple + Ctrl-Reset, or the ROM
  entry) far enough to assert it reports RAM/ROM OK — a strong burn-in for the
  MMU and aux routing.
- Retire the "apple2e returns an error" quirk (#4 in `REWRITE.md`); reconcile
  `TOTAL_RECALL_WRITE_WARNINGS.md` (many of those "unexpected" ][+ writes are
  now *implemented* //e switches).

**Gate:** The self-test headless gate is green; the referenced docs are
updated.

### Phase 8b — ProDOS 80-col + docs (M)

**Goal:** End-to-end ProDOS in 80 columns; user-facing docs.

**Scope:**
- Boot **ProDOS 2.4.3** (already in `disks/`) on the //e and assert its
  80-column Bitsy Bye / `CAT` renders in 80 columns — an aux + 80-col + clock
  integration gate.
- README: add the //e to "What's emulated" and the run examples; fill in the
  parity checklist below (switch by switch, mode by mode).

**Gate:** The ProDOS-80col gate is green; full `cargo test` green; README
verified by following it literally.

---

## Quirks & divergences (record as they are decided)

Seed list; append during implementation, mirroring `REWRITE.md`'s
"Quirks to preserve" / "Documented divergences":

1. **64K //e is a valid config** — before the aux phases (4a–4c), and
   selectable after, the //e runs with no aux card; aux switches are inert and
   aux reads fall back to main. This is real hardware, not a stub.
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

- **ROM redistribution.** ~~The gating dependency for every code phase.~~
  **Resolved:** the Enhanced //e system, video, and keyboard ROMs are in
  `rom/` (PR #217), hashes recorded in Phase 0.
- **Internal-vs-slot ROM arbitration** (`$C100-$CFFF`, `$C300`, `$C800`
  expansion) is the subtlest boot-critical piece — the 80-col firmware lives
  in the internal `$C800` space and must appear/disappear per
  SLOTCXROM/SLOTC3ROM. That is why it is isolated as its own PR (2b); the ROM
  cold-start (2c) and 80-col firmware (5b) gates will catch mistakes.
- **Aux interleave convention** (aux = even columns) — verify against real
  ROM 80-col output before baking golden BMPs (5b).
- **Renderer width migration.** Introducing a 560-wide path risks touching
  shared `scr.rs` code the ][+ golden test depends on. Keep the //e path
  additive (5a); only unify later behind its own gate.
- **Perf of `base_ram_size = 0`.** Routing ZP/stack through a device is
  within budget per the rewrite benches, but re-measure with the //e's
  `mem_bench` if the accelerated (7.16 MHz) mode feels sluggish. Page-pointer
  RAM is the escape hatch (Future work).

## Sequencing notes

- Sub-phases run in order within a theme; the letter is the PR sequence.
  Across themes the dependencies are:
  - **1a/1b** (char ROM) and **2a→2b→2c** (64K bring-up) have no aux
    dependency and can start immediately (ROMs have landed).
  - **3a** needs the glyph sets (1a/1b) and the switch state (2c); **3b** is
    independent of 3a and needs only 2c.
  - The aux phases **4a→4b→4c** are the linchpin. **5b** and all of **6**
    depend on them (5b/6 on 4a+4c). **5a** (the 560 buffer) needs only 3a and
    can land in parallel with Phase 4.
  - **7a** needs the //e renderer (5a, plus 5b for the 80-col screenshot);
    **7b** needs 7a. **8a/8b** come last.
- Every sub-phase keeps the Apple ][+ gates green — that is the regression net.
- Every sub-phase branches from and PRs into `claude/apple-iie-enhanced`, never
  `master` (see the Branch callout at the top and the first ground rule).

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
