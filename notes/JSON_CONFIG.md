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
| B | Real slot flexibility: any slot, multiple Disk ][ controllers, empty slots | M/L | Not started |
| C | "Save current setup" from the palette; `.ewmachine` integration | M | Not started |

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
  card there" is real slot flexibility.
- **Memory addresses are strings**, hex (`"0xd000"`) or decimal
  (`"53248"`); the CLI `--memory` flag stays decimal-only.
- `cpu.speed` and `input.controller` exist only in the config (no new CLI
  flags), wiring to the palette's speed constants and a preferred-name
  gamepad scan at startup.

## What is configurable today (the schema inventory)

| Source | Setting | Values |
|---|---|---|
| CLI | `--model` | `2plus`, `2e` |
| CLI | `--drive1` / `--drive2` | floppy image paths (slot 6) |
| CLI | `--hdd` | ProDOS block image (slot 7) |
| CLI | `--aux` | `80col`, `ext80col`, `ramworksiii[:SIZE]` (//e only) |
| CLI + palette | monitor style | `green`, `amber`, `white`, `rgb` |
| CLI + palette | scanlines | `off`, `light`, `heavy` |
| CLI | `--boot-delay` | seconds |
| CLI | `--fps` | display refresh |
| CLI | `--memory` | `ram\|rom:address:path` regions |
| CLI | `--trace`, `--strict`, `--debug` | debugging |
| palette only | CPU speed | 1.023 MHz (normal), 3.58 MHz, 7.16 MHz — the classic accelerator steps |
| palette only | game controller | picked by name when several are present |
| *(new)* | slots 1–7 | card per slot (below) |

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
  An **absent slot key means empty** — `"empty"` exists to say it
  explicitly. Multiple `"diskii"` entries are legal in the schema (Phase B
  makes them real).
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

### Phase B — real slot flexibility (M/L)

The machine builders currently hard-wire the card set. This phase makes
`Two::new_*` construct from the slot table:

- **Multiple Disk ][ controllers**: `Dsk` instances at `$C080 + slot*16`,
  each with its own P5 boot ROM at `$Cn00` — up to three controllers / six
  drives (the classic maximum). The boot scan order follows the //e/][+
  autostart convention (highest slot first).
- **Empty slots**: no Thunderclock, no Disk II — floating/`0xFF` reads at
  the vacated soft-switch and ROM ranges.
- Card moves (Disk II in slot 4, clock in slot 2, …) fall out of the same
  table-driven construction.
- **Tests**: two controllers with distinct disks both readable (bus-level
  RWTS probes at both slots' addresses); empty slot 6 floats; boot-scan
  order; the whole existing suite green with the default table.

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
- **Boot order with multiple controllers**: autostart scans slot 7 → 1;
  Phase B must decide whether `--drive1`-style "boot this" semantics need
  a `boot` hint in the config. Working assumption: highest populated slot
  wins, as on hardware.
- **Palette state vs config**: monitor/scanlines/speed change at runtime;
  the config sets the *initial* state only (Phase C's save captures the
  current one).
- **`one` (Apple 1 / Replica 1)**: out of scope for now — the schema's
  `machine.model` enum leaves room (`"apple1"`, `"replica1"`) if wanted
  later.
