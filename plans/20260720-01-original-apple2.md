# The Original Apple ][ (`model: apple2`, `builtin:apple2`)

- **Design docs:** this plan carries its own design (memory map, ROM
  set, behavioral differences) — it is self-contained like the telnet
  plan. Background: `notes/JSON_CONFIG.md` (the model/family machinery),
  `notes/REWRITE.md` (how the ][+ machine is built). Primary source:
  the *Apple II Reference Manual* (1978),
  <https://mirrors.apple2.org.za/Apple%20II%20Documentation%20Project/Computers/Apple%20II/Apple%20II/Manuals/Apple%20II%20Reference%20Manual%201978.pdf>.
- **Prerequisite (done):** model tokens spelled out —
  `2plus`→`apple2plus`, `2e`→`apple2e` — so this machine is `apple2`
  (the rename PR, #308).
- **Status:** draft — iterate here before kickoff
- **Target:** `main`; PR granularity decided at kickoff

## Goal

A genuine original **Apple ][** (Integer BASIC, non-autostart Monitor),
distinct from the ][+ we ship today:

```
ewm two --config builtin:apple2       # 48K, no language card, Disk ][ (2 drives)
```

Reset drops to the Monitor `*` prompt; `Ctrl-B` enters Integer BASIC;
`PR#6` (or `C600G` from the Monitor) boots a disk — the 1978 machine,
faithfully, because the original ROM has no Autostart.

## The machine (from the supplied ROMs)

`roms/AppleII/` (owner-supplied; provenance the owner's, same stance as
the committed `341-00xx` ][+ and `342-03xx` //e ROMs):

| Socket | File (`roms/AppleII/…`) | Maps to |
|---|---|---|
| D0–D7 | `Apple Programmer's Aid #1 ROM (D000) - 341-0016 - 2716.bin` | `$D000-$D7FF` |
| D8–DF | *(none supplied — socket empty on this build)* | `$D800-$DFFF` unmapped |
| E0–E7 | `…341-0001 - Integer BASIC.bin` | `$E000-$E7FF` |
| E8–EF | `…341-0002 - Integer BASIC.bin` | `$E800-$EFFF` |
| F0–F7 | `…341-0003 - Integer BASIC.bin` | `$F000-$F7FF` |
| F8–FF | `…341-0004 - Original Monitor.bin` | `$F800-$FFFF` |
| char | `Apple II Character ROM - 341-0036.bin` | video — **byte-identical to the committed `roms/3410036.bin`; reuse it, do not embed a duplicate** |

So the main ROM window is `$D000-$FFFF` with a **hole at `$D800-$DFFF`**
(the ][+ fills that with Applesoft; the original ][ leaves it floating).

### How it differs from the ][+ (`2plus`), and why each is free or cheap

- **Integer BASIC + original Monitor** instead of Applesoft + Autostart.
  Just a different `$D000-$FFFF` ROM image fed to the same machine — the
  ][+ path (`Two::new_2plus`) already maps a `$D000-$FFFF` region.
- **No Autostart** → reset lands at the Monitor, no slot scan, no
  auto-boot. This is *inherent in the ROM* (its reset vector points at
  the Monitor), so no emulator code changes — only the boot **test**
  differs (type `C600G` / `PR#6` instead of expecting an auto-boot).
- **`$D800-$DFFF` hole**: nothing mapped there; reads float, exactly as
  the existing unmapped-read behavior on the ][+ for empty slots.
- **Character ROM identical** → no `scr.rs`/`chr.rs` change; uppercase
  only, as the ][+ already renders.
- **Language card**: the LC worked in the original ][ too, banking the
  same `$D000-$FFFF` window — but if present it must fall back to the
  *Integer* ROM, not Applesoft. The minimal machine has no LC, so this
  is a **kickoff decision**: allow `slot 0` on `2` (wire the Integer ROM
  behind the LC) or reject it for now (the minimal config never needs
  it). Recommendation: reject slot 0 on `apple2` in phase A2, add LC support
  only if wanted — keeps the first landing small and faithful to the
  minimal machine.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| A1 | Embed the `roms/AppleII/` set (statics + provenance/hash test + char-ROM reuse); document the memory map | S | Not started |
| A2 | `config::Model::Two` (`"2"`) + `Two::new_2` from the Integer ROM set; `TwoType::Apple2` becomes buildable; family/validation plumbing; boot-to-Monitor / Integer-BASIC gate + ROM-region golden test | M | Not started |
| A3 | `configs/minimal-apple2.json` builtin; `PR#6` boots DOS 3.3 gate; README + `notes/JSON_CONFIG.md` docs | S/M | Not started |

A1 → A2 → A3 in order. **A1 could fold into A2** (the ROM statics are
only useful once `new_apple2` exists) — decide at kickoff. Every phase: the
standard gates (`fmt`, `clippy -D warnings`, full `cargo test` incl.
golden-BMP) plus the phase gate below.

### A1 — ROM assets

- Five `include_bytes!` statics for `$D000-$D7FF` and `$E000-$FFFF`
  (Programmer's Aid, three Integer BASIC pages, Original Monitor). No
  new char ROM — reuse `CHR_ROM`.
- A test pins the images by SHA-1 (the crate's own `ws::sha1`), recording
  provenance the way the //e ROMs were, and asserts the AppleII char ROM
  equals the committed `3410036.bin` so the reuse can't silently drift.
- **Gate:** the hash/provenance test; tree still builds (the statics are
  unused until A2, so gate A1 with `#[allow(dead_code)]` or land A1+A2
  together — the fold-in question).

### A2 — The `apple2` machine

- `config::Model` gains a variant with serde token `"apple2"`; `Model::two_type()`
  → `TwoType::Apple2` (already in the enum); `family()` = apple2.
- `Two::new_apple2(slot0, slots)` mirrors `new_2plus`: assemble the
  `$D000-$FFFF` ROM (Programmer's Aid at `$D000`, `$D800-$DFFF` left
  unmapped, Integer BASIC `$E000-$F7FF`, Monitor `$F800-$FFFF`) and map
  it. `build_machine`'s `TwoType::Apple2 => Err("unsupported")` arm
  (two.rs) becomes the real constructor.
- Validation: `apple2` is apple2-family (slots/aux/display all apply); the
  slot-0 ruling per the kickoff decision above.
- Convenience-flag path: `--model 2` already parses to `TwoType::Apple2`
  (two.rs pass 2) — confirm it now builds instead of erroring.
- **Gate:** a `two_apple2_boot.rs` test (mirroring `two_boot.rs`):
  reset lands at the Monitor `*` prompt; `Ctrl-B` + `PRINT` exercises
  Integer BASIC; a golden test pins the assembled `$D000-$FFFF` region
  (with the `$D800-$DFFF` hole) byte-for-byte.

### A3 — The built-in config and docs

- `configs/minimal-apple2.json`: `model: apple2`, no slot 0 (48K, no language
  card), a Disk ][ in slot 6 (two drives available). Self-contained (no
  media) — the self-containment sweep already covers it. **Naming
  decision:** `builtin:minimal-apple2` (the user's name) and/or a
  `builtin:2` model-token default. Recommendation: ship
  `minimal-apple2` as asked; add `builtin:2` only if a stock default
  is wanted (kickoff).
- `builtin:list` and the name/self-containment tests extend to it.
- **Gate:** a boot test — `builtin:apple2` + the DOS 3.3 disk in
  drive 1, `C600G` (or `PR#6`) boots to the DOS prompt (the original ][
  analog of the ][+ auto-boot test). README `two` profiles + schema
  inventory updated.

## Hazards

- **ROM sourcing is done** (owner supplied `roms/AppleII/`) — the usual
  blocker for a new model is already cleared. Provenance is the owner's,
  consistent with the existing committed ROMs.
- **The `$D800-$DFFF` hole** is the one subtle map detail: it must be
  *unmapped*, not zero-filled, and the golden ROM-region test should
  assert the machine reads it as the unmapped-read value, not as ROM.
- **Language card behind the Integer ROM** — deferred by the kickoff
  recommendation; if ever added, the LC's ROM-bank fallback must use the
  Integer image, and `new_2plus`'s LC path is the template.
- **`--model 2` already half-exists** (`TwoType::Apple2`, the `"2"`
  parse, the unsupported-error arm): grep for every `TwoType::Apple2`
  and `Apple2 =>` site so none is left returning an error or a wrong
  string once the machine is real.
- **Autostart assumptions in tests/docs**: the ][+ boot flow auto-boots;
  the `2` docs and tests must show the manual `PR#6` path, or a user
  reports "it doesn't boot my disk."

## Decisions to make at kickoff

1. **Language card on `2`** — reject slot 0 for now (recommended,
   smaller) or wire the Integer ROM behind an LC.
2. ~~Naming~~ — settled: model `apple2`, `builtin:apple2` (name =
   token, per the 2plus→apple2plus rename in the prerequisite PR).
3. **Phase granularity** — fold A1 into A2, or keep the ROM-assets phase
   separate.
4. **PR granularity** — one PR per phase (default) or the whole plan in
   one.

## Backlog (recorded, out of scope)

- **Apple2-family component descriptions** (the one-family treatment for
  `two`: CPU/ROM as `machine.memory`) — would make `2` vs `2plus` a ROM
  swap in the document instead of a baked model. Bigger; this plan takes
  the narrow baked-ROM path that matches today's apple2 family.
- **Programmer's Aid #1 / other `$D000` cards** as selectable ROMs.
- **Cassette interface** — the original ]['s native load path (shared
  interest with the Apple 1 cassette backlog item).
