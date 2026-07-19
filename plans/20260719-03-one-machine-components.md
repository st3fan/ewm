# One Machine Components: CPU, RAM, and ROMs in the Document

- **Design doc:** `notes/APPLE1.md` (the real memory maps, the ROM
  forensics answering the Krusader/BASIC question, the embedded ROM
  set and its provenance — read it first) and `notes/JSON_CONFIG.md`
  (the document model this extends).
- **Status:** complete — all phases landed (one PR per phase)
- **Target:** `main`; PR granularity decided at kickoff

## Goal

The one-family profiles describe the machine like the schematics do —
CPU, RAM banks with sizes and addresses, ROM images with addresses —
instead of hiding everything behind `machine.model`:

```json
{
  "description": "Replica 1 — 65C02, 32KB RAM, Integer BASIC + Krusader 1.3 in ROM",
  "machine": {
    "model": "replica1",
    "cpu": "65C02",
    "memory": [
      { "type": "ram", "address": "0x0000", "size": "32k" },
      { "type": "rom", "address": "0xe000", "path": "builtin:apple1-basic" },
      { "type": "rom", "address": "0xf000", "path": "builtin:Krusader-1.3-65C02" }
    ]
  }
}
```

```json
{
  "description": "Classic Apple 1 — 6502, 4KB+4KB RAM, Woz Monitor; Integer BASIC preloaded",
  "machine": {
    "model": "apple1",
    "cpu": "6502",
    "memory": [
      { "type": "ram", "address": "0x0000", "size": "4k" },
      { "type": "ram", "address": "0xe000", "path": "builtin:apple1-basic" },
      { "type": "rom", "address": "0xff00", "path": "builtin:WozMon" }
    ]
  }
}
```

ROM images embed in the binary and resolve as `builtin:<name>` in a
memory region's `path` — the same source-resolution convention
`--config` uses — or as a real file path for user-supplied images. The
only fixed hardware is the PIA at `$D010-$D013`.

Faithfulness decisions baked into those profiles (rationale in
`notes/APPLE1.md`):

- **Apple 1**: BASIC arrives as a *preloaded RAM bank* at `$E000` —
  the real machine cassette-loaded it into RAM; we skip the cassette,
  not the writability. RAM is the chapter-7 map (4KB + the `$E000`
  bank), not today's flat 8KB.
- **Replica 1**: the Krusader slice keeps **its own monitor page** at
  `$FF00` (byte-faithful to the real 8KB ROM); it is *not* composed
  with the pristine WozMon. Pairing the 65C02 CPU with the 65C02
  Krusader build also fixes today's silent mismatch (we ship the 6502
  build on a 65C02).
- `machine.model` stays: family, terminal behavior (the Apple 1's
  7-bit display masking), and the default component set for a bare
  `{"machine": {"model": "apple1"}}`.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| R1 | `roms/`: the four mountable images, the embedded ROM registry, `builtin:` resolution in region paths | M | Done |
| R2 | Config: `machine.cpu`, RAM-bank regions (`size`), validation (overlap, reset vector, family) | M | Done (reset-vector check deferred to R3 — see below) |
| R3 | `One` builds from components; profiles rewritten; byte-identity + boot gates | M/L | Done |
| R4 | Docs: README profiles with CPU/RAM/ROM detail; as-built notes | S | Done |

R1 → R2 → R3 in order; R4 last. Standard gates every phase, plus
`readme_examples_parse`.

### R1 — ROM assets and the registry

- **`rom/` renames to `roms/`** (kickoff decision 4; recommend the
  wholesale rename — one mechanical sweep of the `include_bytes!` /
  test paths, no second ROM directory to explain).
- The four mountable images land as committed files (name = stem =
  token, like `configs/`): `WozMon.rom` (= today's `apple1.rom`),
  `apple1-basic.rom` (already extracted), and the two
  `Krusader-1.3-*.rom` **4KB `$F000-$FFFF` slices** cut from the 8KB
  distributions. The historical `apple1.rom`/`krusader.rom` files
  retire (git history keeps them); a **concatenation test** pins
  provenance: `apple1-basic.rom + Krusader-1.3-6502.rom` ==
  the old 8KB `krusader.rom`, byte for byte.
- An embedded ROM registry in `config.rs` (`include_bytes!` table,
  sorted, like `BUILTINS`): `rom_builtin(name) -> Option<&'static
  [u8]>`, unknown-name errors listing the available names.
- **`builtin:` resolution in memory-region paths**: a region `path`
  starting with `builtin:` resolves against the registry instead of
  the filesystem (escape hatch `./builtin:x`, as for configs);
  `referenced_files` treats `builtin:` paths as non-files, so built-in
  configs may carry ROM references and stay self-contained.
- **Gate:** registry unit tests (resolution, unknown-name listing,
  sorted table); the concatenation/provenance test; the tree-wide
  `rom/`→`roms/` rename proven by the untouched suite (Dormann,
  golden-BMP, boot tests all read ROM paths).

### R2 — Config: `machine.cpu` and RAM-bank regions

- `machine.cpu`: `"6502" | "65C02"` — optional, one-family only (the
  apple2 family's CPU stays a model property; `cpu` joins the family
  rejection table for two). Default when absent: the model's CPU.
- `machine.memory` regions generalize: **exactly one of `path` or
  `size`** —
  - `{type, address, path}` — an image (file or `builtin:`), RAM or
    ROM, as today;
  - `{"type": "ram", address, size}` — an empty RAM bank (`"4k"`,
    `"32k"`, or decimal bytes); `size` with `type: "rom"` is invalid.
- New one-family validation in the completeness pass:
  - regions must not overlap each other or the PIA at `$D010-$D013`;
  - some region must cover the reset vector (`$FFFC-$FFFD`) — a
    machine with no reset vector cannot boot, so it fails validation
    with a message saying exactly that. *(As built: deferred to R3 —
    in R2 document regions are still extras on top of the model's
    default ROMs, so "nothing covers the vector" cannot be judged
    until R3's whole-board semantics land.)*
  (The apple2 family keeps its existing rules; its `memory` regions
  remain extras on top of a model-defined board and are not
  overlap-checked here.)
- Both schemas regenerate.
- **Gate:** validation unit tests (cpu family rejection, path/size
  exclusivity, overlaps, reset-vector coverage, unknown builtin ROM);
  schema golden test.

### R3 — `One` built from components

- `One::new(model)` becomes `One::from_components(cpu, regions)` (the
  PIA fixed; base RAM is just a region), with `model` supplying the
  default component set when the document names none — a bare
  `{"machine": {"model": "replica1"}}` and today's bare `ewm one`
  build the same machine as `builtin:replica1`.
- `configs/apple1.json` and `configs/replica1.json` rewritten to the
  fully spelled profiles above; `one`'s `apply_config` /
  `options_to_config` carry the new fields (round-trip preserved —
  `builtin:` tokens print back as `builtin:` tokens).
- **Gates:**
  - *byte-identity*: the composed `builtin:replica1` memory
    `$E000-$FFFF` equals the real 8KB 65C02 Krusader image; composing
    with the 6502 slice reproduces the historical `krusader.rom`
    machine exactly;
  - the existing Woz-monitor boot tests pass on both profiles (the
    Krusader `E000.E00F` dump test keeps working — BASIC's bytes are
    unchanged);
  - a BASIC smoke test on `builtin:apple1` (`E000R` into BASIC, since
    the profile now preloads it);
  - `--print-config` round trips a component-described machine.

### R4 — Docs

- README: the `one` profiles section gains the CPU/RAM/ROM detail
  (the original complaint) — the JSON now *is* the interesting
  content; a line on `builtin:` ROMs and user ROM paths.
- `notes/APPLE1.md` as-built updates; `notes/JSON_CONFIG.md` schema
  inventory (`machine.cpu`, RAM banks — one-family keys).
- **Gate:** `readme_examples_parse`; standard gates.

## Hazards

- **The `rom/`→`roms/` rename** touches `include_bytes!` in several
  modules and test fixture paths — mechanical, but the whole suite
  must stay green in the same commit.
- **Reset-vector and overlap validation** must not reject the model
  *defaults* (they are generated from the same component sets as the
  builtins — one construction path, tested).
- **`builtin:` in `path`** must never hit the filesystem — pinned by a
  test with a poisoned CWD-relative `builtin:x` file.
- **Byte-faithfulness over composition purity**: resist splitting the
  Krusader monitor page out "for cleanliness" — the plan's whole point
  is that the composed bytes match the real ROMs (`notes/APPLE1.md`).
- The user-facing name switch (65C02 Krusader on the Replica 1) changes
  the `$F000-$FFFF` bytes vs today; anything pinned to the old build
  (trace tests? none known) surfaces in R3's identity gate.

## Decisions to make at kickoff

1. **Apple 1 RAM map** — faithful 4KB + 4KB-at-`$E000` (recommended)
   or keep today's flat 8KB at `$0000`.
2. **BASIC on the Apple 1** — preloaded RAM bank (recommended;
   cassette-faithful, writable) or ROM.
3. **Replica 1 monitor page** — Krusader's own (recommended;
   byte-faithful) or composed pristine WozMon at `$FF00`.
4. **`rom/` → `roms/`** wholesale rename (recommended) or a separate
   directory for the mountable set only.
5. **Krusader slices as committed files** (recommended; name = bytes,
   provenance test) or runtime slicing of the full 8KB images.
6. **PR granularity** — per phase (default) or one PR.

## Backlog (recorded, out of scope)

- **`builtin:list` for ROMs** (`--config builtin:list` shows configs;
  a ROM listing needs a home).
- **Applesoft Lite** (`roms/applesoft-lite.bin`, 8KB) as a mountable
  builtin — a real Replica 1 alternative ROM.
- **Cassette interface** — the faithful way to get BASIC into an
  Apple 1; would make the preloaded-RAM convenience optional.
- **Component descriptions for the apple2 family** (CPU choice, ROM
  swaps) — a much bigger surface; nothing here precludes it.
