# The Original (Unenhanced) Apple //e — `apple2e`, and the `apple2e`→`apple2enhanced` rename

- **Design docs:** this plan is self-contained; background in
  `notes/JSON_CONFIG.md` (model/family machinery), `notes/APPLE_IIE_ENHANCED.md`
  (the //e memory/soft-switch map — the *machine* is shared, only the
  CPU and ROMs differ), and `plans/20260720-01-original-apple2.md` (the
  original-][ work this mirrors). IDEAS.md already lists this as
  "NMOS //e (unenhanced)".
- **Status:** in progress — E1 (the `apple2e`→`apple2enhanced` rename),
  E2 (unenhanced ROM assets), and E3 (the original //e machine) landed; E4
  (the `builtin:apple2e` config) remains
- **Target:** `main`; one PR per phase unless decided otherwise

## Goal

Our current `apple2e` is really the **Enhanced //e** (65C02 + the
342-0303/0304 system ROMs + the 342-0265 MouseText video ROM). So:

1. **Rename** it to `apple2enhanced` — the honest name.
2. **Add a true original (unenhanced) //e** as `apple2e`: a **6502**
   with the unenhanced system + video ROMs, on the same //e hardware.

```
ewm two --config builtin:apple2e         # the 1983 //e: 6502, no MouseText
ewm two --config builtin:apple2enhanced  # the 1985 //e: 65C02, MouseText
```

Naming mirrors `apple2` (original ][) / `apple2plus`: the base name is
the original machine, the suffix is the variant.

## The two machines are the same hardware, different CPU + ROMs

The //e memory map, MMU/IOU soft switches, aux slot, and slot layout are
identical between original and Enhanced — the emulator already models all
of it (`new_2e`). The *only* differences:

- **CPU**: original = NMOS **6502**; Enhanced = **65C02** (adds the WDC
  opcodes — software that uses them won't run on the original, faithfully).
- **System ROM**: `342-0135-B` (CD) + `342-0134-A` (EF), unenhanced —
  vs the Enhanced `342-0304-A` / `342-0303-A`.
- **Video/character ROM**: `342-0133-A` (unenhanced, **no MouseText**)
  vs `342-0265-A` (Enhanced, with MouseText). This is the one *visible*
  difference and the one real complication (below).

So `builtin:apple2e` is the *same slot layout* as today's
`builtin:apple2enhanced` (ext80col aux, Liron@5, Disk ][ @6, RGB
monitor) — just on the 6502 + unenhanced ROMs.

Supplied ROMs (owner, in `roms/AppleIIe/`; provenance the owner's, like
the //e Enhanced and Apple ][ ROMs):
`Apple IIe CD Unenhanced - 342-0135-B - 2764.bin`,
`Apple IIe EF Unenhanced - 342-0134-A - 2764.bin`,
`Apple IIe Video - Unenhanced - 342-0133-A - 2732.bin`.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| E1 | Rename `apple2e` → `apple2enhanced` (token, config file, BUILTINS, tests, schema, docs) — no ROMs | S/M | Done |
| E2 | Embed the `roms/AppleIIe/` set (3 statics + provenance test); the unenhanced video ROM into `chr.rs` | S | Done |
| E3 | The original //e machine: `Model`/`TwoType` variant, `new_2e` selects 6502 + unenhanced ROMs + unenhanced video ROM (renderer picks the //e char ROM by variant); boot gate + ROM golden + the no-MouseText char difference | M/L | Done |
| E4 | `builtin:apple2e` config (6502, ext80col, Liron@5, Disk ][ @6, RGB) + boot gate + docs | S | Not started |

E1 is independent and lands first (like the `2plus`→`apple2plus`
rename, #308). E2 → E3 → E4 in order. E2 may fold into E3. Every phase:
standard gates + `readme_examples_parse`.

### E1 — the rename

Mechanical, mirroring #308: `config::Model` token `apple2e`→`apple2enhanced`
(and `Model::token()`); `git mv configs/apple2e.json → apple2enhanced.json`
with its `title`/`description`/model updated; the `BUILTINS` table
(re-sort: `apple1, apple2, apple2enhanced, apple2plus, replica1`); every
`"apple2e"` literal in tests/error strings; both schemas; README,
`notes/JSON_CONFIG.md`, `examples/myiie.json`. Word-boundary-guarded
replacement (the token appears as `"apple2e"`, `builtin:apple2e`,
`apple2e.json`). Internal Rust identifiers (`TwoType::Apple2E`,
`Model::TwoE`) are the *Enhanced* machine and stay for now — E3 decides
their final names.

**Gate:** `builtin:list` shows `apple2enhanced`; the old `apple2e` errors
with the available list; standard gates + README check.

### E2 — ROM assets

Three `include_bytes!` statics for the unenhanced system halves and the
unenhanced video ROM; a provenance test pinning each by SHA-1
(`ws::sha1`). The video ROM joins `chr.rs` as a second //e character ROM
(`CHR_ROM_IIE_UNENHANCED`) alongside the Enhanced `CHR_ROM_IIE`. Statics
`#[allow(dead_code)]` until E3 if landed separately.

**Gate:** the provenance test; the ROMs commit (mind `.gitignore`'s
`*.bin` — the `!roms/**/*.bin` negation already covers `roms/AppleIIe/`).

### E3 — the original //e machine

- `config::Model` gains the original //e; the Enhanced keeps its variant.
  `TwoType` gains a variant for the 6502 //e (today: `Apple2/Apple2Plus/Apple2E`).
  Naming decided at kickoff — recommend `Model::TwoE` = original (token
  `apple2e`), `Model::TwoEEnhanced` = Enhanced (token `apple2enhanced`),
  renaming the existing `TwoE` → `TwoEEnhanced` (semantic: the plain //e
  is the original). Same for `TwoType`.
- `new_2e` takes the variant: 6502 + unenhanced system ROMs, or 65C02 +
  Enhanced — one constructor, two ROM/CPU sets. **The character
  renderer selects the //e video ROM by variant** — the real
  complication, since `chr.rs`/`scr.rs` render the //e text from a single
  `CHR_ROM_IIE` today.
- **Gate:**
  - the Enhanced //e is **byte-identical** after the refactor — the
    golden-BMP //e screenshots are the tripwire (they must not move);
  - a golden test pins the composed original-//e ROM region;
  - a boot test: the original //e boots to its `]` prompt on the 6502;
  - a char test proves the MouseText range renders as the original
    (inverse uppercase) glyphs, distinct from the Enhanced MouseText.

### E4 — the builtin and docs

`configs/apple2e.json`: `model: apple2e`, `aux: ext80col`, Liron@5,
Disk ][ @6, RGB — the original //e (same layout as apple2enhanced).
Registered in `BUILTINS`; the self-containment/name/list tests extend.
Boot-a-disk gate. README "What's emulated" + the `two` profiles section
gain the original //e (and note the Enhanced is now `apple2enhanced`);
`notes/JSON_CONFIG.md` model inventory; IDEAS.md "NMOS //e" marked done.

## Hazards

- **The //e character ROM split is the one real risk.** Today a single
  `CHR_ROM_IIE` renders every //e; introducing the unenhanced video ROM
  means the renderer must pick per machine. The Enhanced golden-BMP
  screenshots must stay byte-identical — that is the gate that proves the
  split didn't disturb the Enhanced path.
- **CPU faithfulness**: the original //e is a 6502 — 65C02 opcodes must
  *not* execute (the emulator already models both cores; just select
  6502). Worth a test that a 65C02-only opcode traps as illegal on the
  original //e but runs on the Enhanced.
- **Rename churn**: `apple2e` is pinned in more tests than the last
  rename (the //e has its own `two_e_*.rs` suite). E1 sweeps them; the
  suite is the tripwire.
- **`341-0868 …1986-1991…`** in `roms/` is unrelated (an Enhanced-era
  aggregate); do not confuse it with the unenhanced set.

## Decisions to make at kickoff

1. **Rust identifier naming** — `TwoE`/`TwoEEnhanced` (recommended:
   plain = original) vs keeping `TwoE` = Enhanced and adding
   `TwoEUnenhanced`.
2. **Phase granularity** — fold E2 into E3, or keep ROM assets separate.
3. **PR granularity** — per phase (default) or fewer.

## Backlog (recorded, out of scope)

- **Keyboard/other //e ROM variants** (the 342-0132-x keyboard encoders
  we already ship differ by revision).
- **Apple //c** — the MMU/IOU cousin, already an IDEAS item.
