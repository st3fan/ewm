# ROMS — a consistent ROM catalog (SKU-keyed)

**Status:** idea / design note. Not yet a plan. Captured 2026-07-20.
Parked "for later" — nothing here is implemented.

## The idea (in one line)

Give every ROM one canonical name based on **Apple's part number
(SKU)** — e.g. `342-0304-A` — and let both the `roms/` folder *and* the
code refer to ROMs by that SKU, ideally through a single lookup table
`SKU → (description, bytes)`.

## Why

Two things are inconsistent today, and they feed each other:

1. **The `roms/` folder** mixes at least five naming styles, three file
   extensions, and has duplicates + double-extensions (inventory below).
2. **The code embeds ROMs two different ways.** Apple ][ / //e machine
   ROMs are hard-wired with `include_bytes!` + hand-named statics in
   `two.rs` / `chr.rs`; the Apple 1 family ROMs go through a
   `builtin:<stem>` lookup table (`config::ROM_BUILTINS`). A config can
   name `builtin:WozMon` but *cannot* name the //e CD ROM — that one only
   exists as a `static` inside `two.rs`.

A SKU is the natural stable key: it is what Apple silk-screened on the
chip, it is unique, it never changes, and it already appears (in three
different spellings) across the tree.

## Apple's part-number scheme (the target vocabulary)

```
342 - 0304 - A            2764
└┬┘   └─┬─┘  └┬┘          └─┬─┘
 │      │     │             └ EPROM/chip type: 2716=2K 2732=4K 2764=8K 2513=char
 │      │     └ revision letter (A, B, C, D…) — omitted on early parts
 │      └ part number
 └ prefix: 341 = Apple ][ / ][+ era, 342 = //e / //c era
```

So `342-0304-A` = the Enhanced //e CD ($C000–$FFFF upper) ROM, rev A,
in a 2764. `341-0020` = the ][+ Autostart Monitor, no revision suffix.

## Current inventory (as surveyed 2026-07-20)

**Referenced by code** (embedded via `include_bytes!` or reachable as
`builtin:`):

| File (under `roms/`) | Bytes | Where | Naming style |
|---|---|---|---|
| `341-0011.bin` … `341-0015.bin`, `341-0020.bin` | 2048 ea | `two.rs` (][+) | bare SKU, no desc |
| `3410036.bin` | 2048 | `chr.rs`, `two.rs` | **SKU without dashes** |
| `Apple IIe CD Enhanced - 342-0304-A - 2764.bin` | 8192 | `two.rs` | full desc + SKU + chip |
| `Apple IIe EF Enhanced - 342-0303-A - 2764.bin` | 8192 | `two.rs` | full desc + SKU + chip |
| `Apple IIe Video - Enhanced - 342-0265-A - 2732.bin` | 4096 | `chr.rs` | full desc + SKU + chip |
| `AppleII/Apple II ROM Pages E0-E7 - 341-0001 - Integer BASIC.bin` (+0002/0003/0004) | 2048 ea | `two.rs` (][) | subfolder, full desc |
| `AppleII/Apple Programmer's Aid #1 ROM (D000) - 341-0016 - 2716.bin` | 2048 | `two.rs` | subfolder, full desc |
| `AppleII/Apple II Character ROM - 341-0036.bin` | 2048 | `two.rs` test | subfolder — **dup of `3410036.bin`** (same sha1) |
| `Krusader-1.3-6502.rom`, `Krusader-1.3-65C02.rom` | 4096 ea | `ROM_BUILTINS` | product name, `.rom` |
| `WozMon.rom` | 256 | `ROM_BUILTINS` | product name, `.rom` |
| `apple1-basic.rom` | 4096 | `ROM_BUILTINS` | product name, `.rom` |

**Present but NOT referenced by code** (verify before deleting — some may
be loaded by path from tests or kept as provenance):

| File | Bytes | Note |
|---|---|---|
| `AppleIIe/Apple IIe CD Unenhanced - 342-0135-B - 2764.bin` | 8192 | **wanted soon** — the original-//e work (`plans/20260720-02-original-iie.md`) |
| `AppleIIe/Apple IIe EF Unenhanced - 342-0134-A - 2764.bin` | 8192 | same |
| `AppleIIe/Apple IIe Video - Unenhanced - 342-0133-A - 2732.bin` | 4096 | same |
| `Apple IIe Keyboard - 341-0150-A / 342-0132-B / -C / -D - 2716.bin` | 2048 ea | keyboard decode ROMs, unused |
| `341-0868 Apple Computer, Inc. 1986-1991 W5.bin` | 32768 | unclear provenance |
| `Signetics 2513 Video ROM.bin` | 1024 | char generator, unused |
| `a2p.rom` | 12288 | looks like a combined ][+ image |
| `applesoft-lite.bin` | 8192 | unused |
| `Krusader-1.3-6502.rom.bin`, `Krusader-1.3-65C02.rom.bin` | 8192 ea | **double extension**, different size from the `.rom` actually embedded |
| `6502_functional_test.bin`, `65C02_extended_opcodes_test.bin` | 65536 ea | Klaus test suites — likely loaded by path in a test; confirm |

**Problems this surfaces:**
- Five naming styles: bare SKU / no-dash SKU / full-desc+SKU+chip /
  product-name / subfolder-nested.
- Three extensions (`.bin`, `.rom`, `.rom.bin`) for the same kind of thing.
- At least one exact duplicate (`3410036.bin` ≡ `341-0036` in `AppleII/`).
- `.rom` vs `.rom.bin` Krusader pairs differ in size — confusing.
- Config can only reach Apple-1-family ROMs via `builtin:`; the //e/][+
  ROMs are locked inside `two.rs`.

## Proposed target

### 1. One filename convention

Canonical stem = the SKU, optionally with a human tail, one extension:

```
342-0304-A — Apple IIe CD Enhanced (2764).bin
341-0020 — Apple II+ Autostart Monitor (2716).bin
```

SKU first so the folder sorts by part number and the SKU is greppable.
`3410036.bin` → `341-0036 …`. Pick **one** extension (`.bin`). Retire the
`.rom.bin` doubles. Decide whether `AppleII/` / `AppleIIe/` subfolders
stay or flatten (SKU prefix already disambiguates era: 341 vs 342).

For non-Apple ROMs with no SKU (WozMon, Krusader, Apple1 BASIC, the Klaus
test suites) keep the product name — they have no part number, and that's
fine; the catalog can carry them keyed by a short slug instead of a SKU.

### 2. One catalog: `SKU → (description, bytes)`

Generalise today's `config::ROM_BUILTINS` from `(stem, bytes)` into one
table that is the single source of embedded ROMs:

```rust
struct RomEntry {
    sku:  &'static str,   // "342-0304-A"  (or a slug for SKU-less ROMs)
    desc: &'static str,   // "Enhanced //e CD ROM ($C000-$FFFF)"
    data: &'static [u8],  // include_bytes!(…)
}
static ROM_CATALOG: &[RomEntry] = &[ … ];
```

- The `desc` field *is* the "comment about what this ROM is", in data
  instead of a scattered `//` — one place to read the whole ROM story.
- `two.rs` / `chr.rs` stop declaring `static ROM_341_0011: &[u8] = …`
  and look up `catalog("341-0011")` (or keep thin `include_bytes!`
  aliases that the catalog owns — decide during planning).
- `config`'s `builtin:` resolver reads the same catalog, so a machine
  config can finally say `"path": "builtin:342-0304-A"`. Whether builtin
  Apple ][ / //e configs *should* move their ROM wiring out of `two.rs`
  and into `configs/*.json` via `builtin:SKU` is the biggest open
  question — it's the payoff, but it's also the invasive part.

### 3. Consistency in the config surface

Per `CLAUDE.md`: ROM identity is machine data. If configs reference ROMs
by SKU, the `builtin:` names become SKUs, which are documented, stable,
and schema-checkable. `--print-config` would then show exactly which
silicon a machine is running.

## Open questions (need a decision before a plan)

1. **Scope of the code change.** Minimum = folder cleanup + catalog table
   + `builtin:SKU` reachable. Maximum = also move the ][+/][/​//e ROM
   wiring from `two.rs` statics into `configs/*.json`. One PR each, or one
   plan? (`CLAUDE.md`: you decide per-plan at kickoff.)
2. **Subfolders or flat?** `AppleII/` + `AppleIIe/` vs a flat SKU-sorted
   folder.
3. **SKU-less ROMs.** Slug key in the same catalog, or a second small
   table? (WozMon, Krusader, apple1-basic, Klaus suites.)
4. **Orphan ROMs.** Delete the unreferenced/dup/`.rom.bin` files, or keep
   for provenance? (`git log` history preserves them either way.) Confirm
   the Klaus test `.bin`s aren't loaded by path first.
5. **Interaction with in-flight work.** The unenhanced `AppleIIe/` set is
   about to be wired by `plans/20260720-02-original-iie.md`. Sequence the
   reorg *after* that lands, or fold the catalog in as that work adds its
   ROMs, so the new ROMs are born SKU-keyed?

## Hazards

- **Renames must be `git mv`** so history follows, and every
  `include_bytes!` / `std::fs::read` path updates in lock-step — a missed
  one is a compile error (good) but a wrong-but-existing path is a silent
  swap (bad). The golden-BMP screenshot tests are the tripwire that the
  bytes feeding the video path didn't change.
- Filenames with spaces/`#`/`()` already work in `include_bytes!`; keep
  quoting them.
- Don't churn `3410036.bin` and its `AppleII/` twin independently —
  collapse the duplicate as part of the same change.

## Next step

Graduated into `plans/20260720-03-rom-catalog.md` (phases R1 folder rename
→ R2 catalog table → R3 `builtin:SKU` resolver → R4 optional config
migration). The open questions above are carried there as "Decisions to
make at kickoff". Sequenced *after* the original-//e ROMs land so the
reorg happens once. Parked — we'll work on it later.
