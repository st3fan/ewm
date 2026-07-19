# JSON Machine Configuration (`--config`) — Implementation Plan

A working document for configuring a whole machine from one JSON file:

```
ewm two --config myemulator.json
```

In the house style: re-read at the start of every session, update as phases
land. **Every phase passes the full gates** (`cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`).

## Owner's decisions (recorded)

- **JSON, not TOML** — better supported (editors, JSON Schema tooling,
  every language). This supersedes the TOML wording in `MAC_APP.md`
  Phase 3.
- **A JSON Schema** captures *everything* configurable today via the
  command line **and** the Command Palette.
- **Slots are configurable**: no cards, three Disk ][ controllers, the
  different aux cards — the machine's physical layout lives in the file.
- **CLI options stay** — only `--config <path>` is added for now.

## Status

| Phase | Description | Size | Status |
|---|---|---|---|
| A | `--config` + schema + serde types; today's layout expressible | M | **Done** |
| B | Real slot flexibility: any slot, multiple Disk ][ controllers, empty slots | M/L | **Done** |
| Sources | Built-ins, overlays, `--print-config` (`plans/20260718-02-config-sources.md` C1–C5) | M | **Done** |
| C | "Save current setup" from the palette; `.ewmachine` integration | M | Not started — seeded by `options_to_config` (C4) |

## Phase A decisions (recorded as built)

- **Precedence is structural, not tracked**: `parse_options` runs two
  passes — pass 1 loads `--config` files into `Options`, pass 2 is the
  unchanged flag loop, which only assigns a field when its flag is present.
  CLI-overrides-config falls out without per-flag "was it given" state.
  Multiple `--config` flags apply in order; config and CLI `--memory`
  regions are additive.
- **schemars is a dev-dependency only** (owner's decision): the derive is
  gated `#[cfg_attr(test, derive(schemars::JsonSchema))]`, keeping schemars
  out of release builds. Consequence: **no `#[schemars(...)]` attributes on
  the config structs, ever** — doc comments become the schema descriptions.
  Promote it to a plain dependency if an attribute is genuinely needed.
- **Schema regeneration**: the golden-file test keeps
  `schema/ewm-config.schema.json` byte-for-byte in sync; regenerate with
  `EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed`.
- **serde caveat resolved**: `deny_unknown_fields` on the internally tagged
  `SlotCard` enum *does* reject typo'd keys with current serde (the
  `unknown_slot_card_key_is_rejected` canary test pins this).
- **Slot rulings**: slot 7 `"empty"` is accepted (just no HDD attached);
  `"empty"` in slots 1 and 6 is *rejected* with the Phase B message — the
  machine builders hard-wire the Thunderclock and Disk II today, so "no
  card there" is real slot flexibility. *(Superseded by Phase B: any card
  in any slot.)*
- **Memory addresses are strings**, hex (`"0xd000"`) or decimal
  (`"53248"`); the CLI `--memory` flag stays decimal-only.
- `cpu.speed` and `input.controller` exist only in the config (no new CLI
  flags), wiring to the palette's speed constants and a preferred-name
  gamepad scan at startup.

## Phase B decisions (recorded as built)

- **`Two::new_with_slots(model, aux, &BTreeMap<u8, SlotDevice>)`** is the
  table-driven constructor; `Two::new`/`new_with_aux` delegate with the
  default table (Thunderclock@1, Disk II@6), so the whole pre-Phase-B test
  suite runs unchanged. The machine table carries **card kinds only** —
  media inserts afterwards (`load_disk_at(slot, drive, path)`,
  `attach_hdd_at(slot, path)`; the hard drive attaches post-construction
  because `Hdd::new` needs its image up front).
- **Devices decode only the DEVSEL low nibble** (`addr & 0x0f`); each is
  registered over its slot's 16-byte range, so one implementation serves
  any slot. `DSK_ROM` is naturally slot-agnostic (the P5 ROM derives its
  slot from the return address); the clock and hard-disk firmware are
  generated per slot (`clk_rom(slot)`, `hdd_rom(slot)`), with golden tests
  pinning the slot-1/slot-7 images byte-for-byte to the pre-generator
  statics.
- **Boot controller = highest slot with a Disk II**; `load_disk`, the
  drag-drop handler, and `dsk()` target it. No config boot hint — the real
  Autostart ROM scan (7→1) picks the boot device, exactly as on hardware
  (proved by the boot-scan tests in `ewm/tests/two_slots.rs`).
- **Empty slots read `0x00`** (owner's decision), the pre-existing
  unmapped-read behavior on both models — it fails the Autostart boot
  signature just as safely as the floating/`0xFF` this plan originally
  sketched.
- **Multiplicity**: at most three Disk ][ controllers (the classic
  maximum) and one Thunderclock (ProDOS installs a single clock driver);
  **hard drives are unlimited** (owner's decision) — `Two` keeps them in a
  slot-keyed map like the controllers.
- **Absent `machine.slots` = the default layout**; a *present* `slots`
  object (even `{}`) is literal — absent keys inside it are empty slots.
  This keeps `{"machine": {"model": "2plus"}}` equal to bare `ewm two`
  while honoring "an absent slot key means empty". *(The `--drive1`/
  `--drive2`/`--hdd` sugar this section originally described was replaced
  by `--set`; see the CLI overrides decisions below.)*
- **Drive lights are OR'ed across controllers** — the status bar and LED
  strip keep their two-light layout; at any moment at most one controller
  spins, so the pair reads as the active controller's drives.

## CLI overrides (`--set`) decisions (recorded as built)

- **`--set <key>=<value>`** overrides one config value by colon-separated
  key path (`--set machine:slots:6:drive1=game.dsk`, `--set
  display:monitor=amber`). A **separate flag** (owner's decision — not
  overloaded onto `--config`), and the `--drive1`/`--drive2`/`--hdd`
  sugar flags are **removed** (owner's decision); the boo launcher and
  docs speak `--set`.
- **One config document, sources layered left-to-right**: `--config`
  files load through the typed path (per-file validation and relative-path
  resolution intact) and are serialized back to JSON (`serde::Serialize`
  on the config types — fronting Phase C); `--set` mutates the document
  directly. One typed conversion + validation at the end
  (`config::from_document`). Consequence: multiple `--config` files now
  **deep-merge** instead of a later file's slots table replacing
  wholesale. The remaining convenience flags (`--model`, `--color`, …)
  still override the finished document, per the Phase A precedence design.
  *(Since superseded: the per-setting convenience flags were retired by
  `plans/20260719-01-flag-retirement.md`; `--serve` is the one
  document-overriding flag left.)*
- **Merge rules** (`config::merge_documents`): objects merge recursively;
  `null` and empty-array overlays are no-ops (a source that doesn't set a
  field must not clear it); two objects whose `"card"` discriminators
  differ replace wholesale (merging a diskii's drives into an `"empty"`
  card would fail validation). `apply_set` mirrors the card rule: setting
  a different `card` resets the object's other fields.
- **Slots materialization**: a `--set` entering `machine:slots` on a
  document without one materializes the default table first, so `--set
  machine:slots:6:drive1=x` on a bare command line extends the default
  machine exactly like the removed `--drive1` did.
- **Value typing**: a `--set` value that parses as JSON is used as JSON
  (numbers, booleans, quoted strings, whole objects — `--set
  'machine:slots:7={"card":"harddrive","image":"tr.hdv"}'` is the one-line
  `--hdd` replacement); anything else is a plain string. Escape hatch for
  values that accidentally parse as JSON (a file named `123`): quote them
  (`--set 'machine:slots:6:drive1="123"'`). `--set` path values stay
  as-given (CWD-relative), like the flags they replace; file paths keep
  resolving against their config's directory.
- **Array paths are rejected** (`machine:memory:0:path`) — memory regions
  come from `--memory`.

## Config sources — built-ins (C1, recorded as built)

Phase C1 of `plans/20260718-02-config-sources.md`:

- **`--config builtin:<name>`** loads one of the embedded copies of the
  `configs/` files (`include_str!`, a static table in `config.rs` — no
  build script). Names are the schema's model tokens, matching the file
  stems 1:1: `builtin:2plus` (`configs/2plus.json`) and `builtin:2e`
  (`configs/2e.json`; the files were renamed from `plus.json` /
  `enhanced.json`). `builtin:list` prints the names with descriptions
  and exits 0, like `--help`; an unknown name errors listing the
  available names. A literal file named `builtin:x` is reachable as
  `./builtin:x` (documented, not engineered around).
- **Built-ins are self-contained**: no drive images, memory files, or
  trace/state paths — there is no directory to resolve relative paths
  against. `load_builtin` rejects file references at load time and the
  `builtins_load_and_are_self_contained` test pins the property (plus:
  every builtin carries a `description`, and the table stays sorted).
- **Top-level `description`** joined the schema — a one-line human
  description shown by `builtin:list`, usable by any config file.
- **Bare `ewm two` is unchanged** — the in-code default machine
  (Thunderclock in slot 1) still differs from `builtin:2plus`
  deliberately; unifying them is backlog in the plan.

## Config sources — partial configs (C2, recorded as built)

Phase C2 of `plans/20260718-02-config-sources.md`, the enabling change
for `--config-overlay` (C3):

- **The serde types are partial-friendly**: `Config.machine` and
  `Machine.model` became `Option`, so any fragment — `{}`, a slots-only
  overlay — parses. `merge_documents` needed no change (absent options
  serialize to `null`, already a merge no-op).
- **Validation split in `config.rs`**: `validate` is *structural* (per
  file, fragment-judgeable: unknown keys, enum values, slot rules,
  multiplicity, sizes/addresses/ports); `validate_complete` is
  *completeness* (final document only: `machine.model` present, plus the
  model cross-checks — aux is //e-only, the //e has no slot 0 — which an
  overlay can't be judged on until the merged document names the model).
- **Loader contract**: `load`/`load_source_document` (the `--config`
  path) still require completeness per file — a partial file fails with
  `machine.model is required (is this an overlay? use --config-overlay)`
  — while `load_document` runs only the structural pass (with the file
  named in errors and relative paths resolved) and is the path overlays
  will load through. `from_document` runs both passes on the final
  layered document; its missing-model message is
  `machine.model is required (start from --config, e.g. --config
  builtin:2plus)`.
- **Two schemas, one generator** (plan option 2): the generated schema is
  now overlay-shaped, so the golden test post-processes the requiredness
  (`machine`, `machine.model`) back into `schema/ewm-config.schema.json`
  and commits the relaxed one as `schema/ewm-config-overlay.schema.json`
  (own title/description) for editor support of overlay files. One
  regeneration command updates both.

## Config sources — overlays (C3, recorded as built)

Phase C3 of `plans/20260718-02-config-sources.md`:

- **`--config-overlay <source>`** layers a partial config onto the
  document; repeatable, and all sources — the `--config` base, overlays,
  `--set` — apply strictly in command-line order through the existing
  merge. Overlay files load through the structural-only typed path
  (`load_overlay_document` → `load_document`): the overlay file is named
  in errors and its relative paths resolve against its own directory.
  The `builtin:` scheme is shared with `--config` (a complete config is
  a valid overlay); overlay-only command lines start from the default
  machine, like bare `--set`.
- **One `--config` max** (plan recommendation adopted): a second errors
  with `only one --config allowed; use --config-overlay for additional
  layers`. Mildly breaking — two `--config` files used to deep-merge —
  but that read as an accident once partial layers had their own flag.
- **Slots materialization extends to overlays**
  (`merge_overlay_document`): an overlay carrying `machine.slots` onto a
  slotless document materializes the default table first, so
  `--config-overlay hdd7.json` means "the default machine plus a hard
  drive in slot 7". A base's explicit table (from `--config`) stays
  literal — materialization fills a missing table, never touches a
  present one. All four base × overlay combinations are pinned in
  `config.rs` tests; `--set`'s materialization is untouched (the boo
  launcher's drag-drop paths behave identically).
- **Known edge, accepted**: overlaying a complete *//e* config onto the
  slotless ][+ default (`ewm two --config-overlay builtin:2e`) fails
  completeness — materialization brings in the ][+ default table, whose
  slot 0 the //e rejects. Consistent with the rules (an overlay means
  "default machine plus this"); start from `--config builtin:2e`
  instead.

## Config sources — `--print-config` (C4, recorded as built)

Phase C4 of `plans/20260718-02-config-sources.md`:

- **`ewm two … --print-config`** prints the machine the command line
  describes — sources *and* convenience flags applied, i.e. the machine
  a real run would build — as config JSON on stdout and exits 0. Any
  load/validation error still exits nonzero first, so the flag doubles
  as a config linter for scripts and CI.
- **`options_to_config` (two.rs) is the one Options→Config mapping** —
  the inverse of `apply_config`, kept as a single function so the
  palette's "save current setup" (Phase C) reuses it. The full mapping
  was implemented; the plan's fallback (print before convenience flags)
  wasn't needed. `--wozbug`, `--break` and the hidden `--screenshot`
  are debug tooling, not machine configuration, and don't appear.
- **The document is explicit where it matters, quiet where it doesn't**:
  model, slot table, display and cpu settings print even at their
  defaults (stable against future default changes); off-by-default
  extras (strict, debug, boot delay, remote) appear only when enabled.
  `config::compact_document` does the shaping — it drops `null`s, empty
  arrays, and sections that emptied out, but keeps a genuinely bare
  `"slots": {}` (which means "no cards", not "default layout"). Keys
  print in sorted order (documents are sorted maps) — deterministic,
  semantics-free.
- The printed document round-trips: fed back via `--config` it yields
  identical `Options` (pinned by e2e tests, including a composed
  base + overlay + `--set` + flags command line). Path fields are
  emitted as the run would use them, so `--set` paths given relative to
  the CWD print relative and would re-resolve against the printed
  file's directory — save next to where you ran, or use absolute paths.

## What is configurable today (the schema inventory)

Since `plans/20260719-01-flag-retirement.md`, the CLI column is
`--set <key>=<value>` (or any config source) for everything — the
per-setting convenience flags are gone.

| Source | Setting | Values |
|---|---|---|
| config + `--set` | `machine.model` | `2plus`, `2e` |
| config + `--set` | `machine.aux` | `80col`, `ext80col`, `ramworksiii` (+ `size`; //e only) |
| config + `--set` + palette | `display.monitor` | `green`, `amber`, `white`, `rgb` |
| config + `--set` + palette | `display.scanlines` | `off`, `light`, `heavy` |
| config + `--set` | `boot.delay` | seconds |
| config + `--set` | `display.fps` | display refresh |
| config + `--memory` | `machine.memory` | `type`/`address`/`path` regions |
| config + `--set` | `debug.trace`, `cpu.strict`, `debug.enabled` | debugging |
| config + `--set` + palette | `cpu.speed` | 1.023 MHz (normal), 3.58 MHz, 7.16 MHz — the classic accelerator steps |
| config + `--set` + palette | `input.controller` | picked by name when several are present |
| config + `--set` | slots 0–7 | card per slot (below); slot 0 is the ][+ language-card socket |

## Slot 0 decisions (recorded as built)

- **Slot 0 is a `"0"` key in `machine.slots`** (owner's decision —
  hardware-faithful, not a separate field). The ][+ language card was
  hardwired into `Two::new_2plus` before this; now it is a declared card.
- **Card set restricted**: slot 0 takes only `"language"`, `"saturn128"`
  or `"empty"` — it has no `$Cn00` firmware space, so firmware-bearing
  cards can't work there — and those cards fit nowhere else. The //e
  rejects `"0"` outright (its language card is soldered onto the
  motherboard).
- **The literal-table rule covers slot 0** — a deliberate breaking
  change, accepted by the owner: a ][+ config whose `slots` table omits
  `"0"` is a stock **48K machine** ($D000–$FFFF motherboard ROM on the
  bus, slot 0's DEVSEL range unmapped). `configs/2plus.json` declares the
  card explicitly. The default table (absent `slots`, and the `--set`
  materialization) gains `"0": {"card": "language"}`, so bare command
  lines stay the classic 64K build; `--set machine:slots:0:card=empty`
  is the 48K opt-out.
- **Machine plumbing**: slot 0 never becomes a `SlotDevice` —
  `build_machine` consumes it as a `two::Slot0` (Language / Saturn128 /
  Empty) passed to `Two::new_with_slots`. `Two::slot0()` reports it and
  WozBug's `SLOTS` shows the slot 0 line on the ][+ (with the selected
  bank for the Saturn). DOS 3.3 boots on the 48K machine (it just skips
  loading Integer BASIC) — regression-tested.
- **`"saturn128"` — the Saturn Systems 128K RAM Board**
  (`ewm/src/saturn.rs`, from the Saturn Operations Manual ch. 9): eight
  16K banks at $D000–$FFFF, each two 4K banks (A/B) at $D000 plus its
  own 8K. The $C08x A2=0 column is the exact Language Card protocol
  (bank 1 is how DOS/Pascal/VisiCalc see a "16K card"); A2=1 selects the
  16K bank ($C084–7 → 1–4, $C08C–F → 5–8), with read/write/4K state
  persisting across switches. Write-enable follows the LC's read-twice
  rule (the manual's "PEEK or POKE" prose is looser; the LC-compatible
  semantics are what software relies on and what `alc.rs` implements).
  Regression-tested: all eight banks hold independent contents on the
  bus, and DOS 3.3 loads Integer BASIC into bank 1 with INT/FP switching
  both ways.
- **Future**: an Integer BASIC Firmware card would be a fourth slot 0
  card kind; the Saturn 32K/64K siblings would be a size field or
  variants.

## The Liron / UniDisk 3.5 decisions (recorded as built)

- **`"liron"`** (`ewm/src/liron.rs`): the UniDisk 3.5 Controller as
  virtual hardware in the `hdd.rs` style — hand-assembled `$Cn00`
  firmware over magic DEVSEL ports, no IWM emulation. Two drives
  (`drive1`/`drive2`), **.2mg only**, ProDOS-order, exactly 800 (400K)
  or 1600 (800K) blocks; the 2IMG locked flag mounts read-only;
  write-back lands at `data_offset + block*512`, header preserved. Any
  slot 1–7, no multiplicity limit; `configs/2e.json` carries one
  in slot 5.
- **SmartPort identity is real**: signature `$Cn07=$00`, ID type at
  `$CnFB`, ProDOS entry via `$CnFF` with the SmartPort dispatch at
  entry+3. The dispatch implements STATUS (device count, per-unit
  status + block count, DIB), READ_BLOCK and WRITE_BLOCK (translated
  onto the ProDOS driver's pump); everything else returns $21. The
  **Enhanced //e Autostart scan boots it** — the boot test proved the
  scan accepts `$Cn07=$00`, so an 800K ProDOS .2mg in the highest
  populated slot boots exactly like a hard drive.
- **Space economies in the 256-byte firmware** (all safe for this
  media): the third SmartPort block byte is ignored (800K = 1600
  blocks), `$45` is not restored after transfers and `$42-$45` are
  borrowed by the dispatch (ProDOS rebuilds `$42-$47` per call), and
  ProDOS STATUS reports empty drives as 0 blocks rather than an error
  (reads fail with `$2F` where it matters). The boot gap below `$40`
  houses the SmartPort block-call setup.

## The schema (draft)

Draft 2020-12, committed as **`schema/ewm-config.schema.json`**. Sketch of
the shape (the committed schema is the authority; field-level `description`s
throughout so editors show help):

```json
{
  "machine": {
    "model": "2e",
    "aux": { "card": "ramworksiii", "size": "1m" },
    "slots": {
      "1": { "card": "thunderclock" },
      "5": { "card": "diskii", "drive1": "work.woz" },
      "6": { "card": "diskii", "drive1": "dos33.dsk", "drive2": "blank.dsk" },
      "7": { "card": "harddrive", "image": "Total Replay v6.0.1.hdv" }
    },
    "memory": [ { "type": "rom", "address": "0xd000", "path": "custom.bin" } ]
  },
  "display": { "monitor": "green", "scanlines": "off", "fps": 30 },
  "cpu": { "speed": "normal", "strict": false },
  "input": { "controller": "Xbox Wireless Controller" },
  "boot": { "delay": 1.5 },
  "debug": { "trace": "trace.txt", "enabled": false }
}
```

Schema rules:

- `machine.model` required; everything else optional with the current
  defaults (an empty `{ "machine": { "model": "2plus" } }` is today's bare
  `ewm two`).
- **`slots`**: object keyed `"1"`–`"7"`; each value is a card object
  discriminated by `"card"`: `"diskii"` (`drive1`/`drive2` image paths,
  both optional), `"harddrive"` (`image`), `"thunderclock"`, `"empty"`.
  When the whole `slots` object is **absent** the machine gets the default
  layout (clock in 1, Disk II in 6); when **present** it is literal — an
  absent slot key means empty, and `"empty"` says it explicitly. Up to
  three `"diskii"` entries, one `"thunderclock"`, any number of
  `"harddrive"` cards (Phase B).
- `machine.aux`: `{ "card": "80col" | "ext80col" | "ramworksiii", "size":
  "64k".."8m" }` — `size` only valid with `ramworksiii`; whole `aux` object
  only valid with `"model": "2e"` (enforced in code; the schema documents
  it).
- `cpu.speed`: `"normal" | "3.58mhz" | "7.16mhz"` — exactly the palette's
  accelerator steps (`SPEED_NORMAL/FAST/FASTER`).
- `input.controller`: a preferred game-controller name; hot-plug still
  applies when absent or unmatched.
- **`additionalProperties: false` everywhere** — a typo'd key is an error,
  not a silent ignore.
- **Relative paths resolve relative to the config file's directory** — the
  property that makes `.ewmachine` bundles (MAC_APP Phase 3) portable.

## Implementation

### Dependencies (flagged for the owner)

- **`serde` + `serde_json`** for parsing into typed structs with
  `deny_unknown_fields` — the ecosystem standard; this is EWM's first
  general-purpose third-party dependency beyond sdl3/fontdue/chrono.
- **`schemars`** to *derive* the JSON Schema from the same serde structs —
  one source of truth. A unit test regenerates the schema and asserts it
  matches the committed `schema/ewm-config.schema.json` byte-for-byte
  (regenerate with a documented one-liner when the config grows).

### Phase A — `--config`, schema, today's capabilities (M)

- `ewm/src/config.rs`: the serde types mirroring the schema + `pub fn
  load(path) -> Result<Config, String>` (parse + semantic validation +
  path resolution against the config's directory).
- `--config <path>` in `parse_options`: the file populates `Options`
  first; **explicitly given CLI flags override config values** (documented
  precedence — makes quick experiments cheap and keeps `.ewmachine` +
  overrides possible later).
- Phase A accepts only layouts the emulator can already build: slot 6
  `diskii`, slot 7 `harddrive`, slot 1 `thunderclock`, any aux card —
  i.e. today's machine expressed in slot syntax. Any other layout parses
  fine but errors clearly: *"slot 5 diskii: not supported yet (see
  notes/JSON_CONFIG.md Phase B)"*.
- `input.controller` + `cpu.speed` wire to the existing palette
  mechanisms at startup.
- **Tests**: schema/struct agreement (the schemars round-trip); example
  configs parse (a committed `ewm/tests/configs/` set); unknown-key and
  bad-value rejections with useful messages; relative-path resolution; a
  boot gate — a config naming the DOS 3.3 disk boots exactly like
  `--drive1` does; CLI-overrides-config precedence.
- **Docs**: README `--config` section + an example; `MAC_APP.md` Phase 3
  rewritten TOML→JSON pointing here; IDEAS.md config bullet → planned.

### Phase B — real slot flexibility (M/L) — done

The machine builders used to hard-wire the card set; `Two::new_*` now
constructs from the slot table (decisions above):

- **Multiple Disk ][ controllers**: `Dsk` instances at `$C080 + slot*16`,
  each with its own P5 boot ROM at `$Cn00` — up to three controllers / six
  drives (the classic maximum). The boot scan order follows the //e/][+
  autostart convention (highest slot first).
- **Empty slots**: no Thunderclock, no Disk II — the vacated soft-switch
  and ROM ranges read `$00` (not the floating/`$FF` originally sketched;
  see the Phase B decisions).
- Card moves (Disk II in slot 4, clock in slot 2, …) and multiple hard
  drives fall out of the same table-driven construction.
- **Tests** (`ewm/tests/two_slots.rs`): two controllers with distinct
  state and disks both readable (bus-level probes at both slots'
  addresses); empty slot 6 reads `$00` and the machine falls through to
  BASIC; boot-scan order; a clock moved to slot 2 (through a full ProDOS
  boot); hard drives in two slots; the whole existing suite green with
  the default table, untouched.

### Phase C — round-tripping (M)

- Palette: "Save machine configuration…" writes the *current* state
  (monitor, scanlines, speed, mounted disks, aux card) as a valid config
  file — the seed of the `.ewmachine` document (MAC_APP Phase 3).
- `serde::Serialize` on the same structs; a round-trip test
  (save → load → identical machine).

## Risks & open questions

- **`.po` in `slots`**: the floppy/hard-drive ambiguity does not exist
  here — the slot's card type says which is meant. (The drag-drop size
  heuristic in `media.rs` stays for pathless opens.)
- **Boot order with multiple controllers** *(resolved in Phase B)*: the
  Autostart scan (slot 7 → 1) picks the boot device — highest populated
  slot wins, as on hardware. No `boot` hint in the config.
- **Palette state vs config**: monitor/scanlines/speed change at runtime;
  the config sets the *initial* state only (Phase C's save captures the
  current one).
- **`one` (Apple 1 / Replica 1)**: out of scope for now — the schema's
  `machine.model` enum leaves room (`"apple1"`, `"replica1"`) if wanted
  later.
