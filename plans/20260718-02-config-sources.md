# Config Sources: Built-ins, Overlays, and Layering

- **Design doc:** `notes/JSON_CONFIG.md` (the config document model, merge
  rules, `--set` semantics — read it first; this plan builds on the
  "one config document, sources layered left-to-right" architecture it
  records and does not repeat the rationale). Update it with *as built*
  notes as phases land.
- **Status:** complete — all phases landed (one PR per phase)
- **Target:** `main`; one PR per phase unless decided otherwise at kickoff

## Goal

Make the machine configuration surface fully compositional:

```
ewm two --config builtin:enhanced \
        --config-overlay drive-with-total-replay.json \
        --config-overlay vnc.json \
        --set display:monitor=amber
```

Four source kinds, one document, layered in command-line order:

1. **Built-in configs** — the files under `configs/` ship *inside* the
   binary and are selected with `--config builtin:<name>`.
2. **`--config <path>`** — a *complete* user-defined machine config
   (unchanged from today).
3. **`--config-overlay <path>`** — a *partial* config deep-merged on top
   of whatever is loaded so far; repeatable, applied in order.
4. **`--set <key>=<value>`** — single-value overrides, exactly as today.

The convenience flags (`--model`, `--color`, `--serve`, …) keep their
Phase-A precedence: they override the finished document.

## Why this shape

The document-layering engine already exists: `--config` files load
through the typed path and deep-merge via `config::merge_documents`,
`--set` mutates the document, and one `config::from_document` validates
the final result. So `--config-overlay` is *not* a new merge mechanism —
it is a new **source kind** with different *per-file* rules:

- A **config** must be complete (`machine.model` present) and is the
  base of the document.
- An **overlay** may be arbitrarily partial — `{"machine": {"slots":
  {"7": {"card": "harddrive", "image": "tr.hdv"}}}}` is a whole valid
  overlay — but is still *structurally* validated per file (unknown
  keys, enum values, slot rules), with the file named in errors.

Today `load_document` runs the full typed parse per file, so a partial
overlay fails on the missing `machine.model`. That constraint has to
move: parse-level types become partial-friendly, and *completeness*
becomes a final-document check. That is the heart of Phase C2.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| C1 | Built-in configs: `builtin:` scheme, embed `configs/`, listing | S/M | Done |
| C2 | Partial configs: optional `machine`/`model`, split validation | M | Done |
| C3 | `--config-overlay`: flag, layering rules, slots materialization | M | Done |
| C4 | `--print-config`: inspect the merged document | S | Done |
| C5 | Docs sweep + `notes/JSON_CONFIG.md` as-built update | S | Done |

C1 is independent; C2 must land before C3 (overlay loading needs partial
parsing); C4 wants C3 (it is most useful once layering is rich).

### C1 — Built-in configs (`--config builtin:<name>`)

- Embed every file under `configs/` at compile time (`include_str!` via a
  small static table in `ewm/src/config.rs` — no build script, no new
  dependency). `configs/` stays the single source of truth; the binary
  carries a copy.
- `--config` grows a source-resolution step: a `builtin:<name>` argument
  looks up the embedded table; anything else is a file path as today. A
  literal file named `builtin:x` is escapable as `./builtin:x` (document
  it, don't engineer around it).
- Unknown name errors list the available names:
  `no built-in config "foo" (available: 2e, 2plus)`.
- **Built-ins must be self-contained**: a test walks every embedded
  config and asserts it references no files (no drive images, no memory
  regions, no trace/state paths) — the base-directory question then
  never arises. Runtime guard: if a builtin ever carries a relative
  path, loading errors rather than resolving against the CWD.
- Listing: `ewm two --config builtin:list` (or `--list-configs`; pick at
  kickoff) prints each name with a one-line description. Add an optional
  top-level `description` field to the schema so configs can describe
  themselves (schema regenerated; useful for user configs too).
- **Decisions (made at kickoff, as built):**
  - **Names are the schema's model tokens** — `builtin:2plus` and
    `builtin:2e` — and the files were renamed to match 1:1
    (`configs/2plus.json`, `configs/2e.json`), so name = file stem.
  - **Listing is `--config builtin:list`** (no new flag).
  - **Bare `ewm two` is unchanged**; unifying the in-code default
    machine with a built-in stays in the backlog.
- **Gate:** unit tests (resolution, unknown-name message, listing,
  self-containment sweep); an integration test in `ewm/tests/two_config.rs`
  proving `--config builtin:2plus` yields the same machine as
  `--config configs/2plus.json`; full standard gates. *(All in place —
  see `notes/JSON_CONFIG.md` "Config sources — built-ins".)*

### C2 — Partial configs (parse relaxed, validate the whole)

The enabling change for overlays, landed separately so C3 stays small.

- `Config.machine` becomes `Option<Machine>` and `Machine.model` becomes
  `Option<Model>` in the serde types. Every other field is already
  optional. `merge_documents` needs no change: absent options serialize
  to `null`, which the merge already treats as a no-op.
- `validate()` splits:
  - **Structural** (per file): unknown fields (serde), enum values,
    slot/aux/memory/remote rules — everything that can be judged on a
    fragment. Runs in `load()`/`load_document()` with the file named in
    the error, exactly as today.
  - **Completeness** (final document only): `machine.model` present.
    Runs in `from_document()`. Error text points at the fix:
    `config: machine.model is required (start from --config, e.g.
    --config builtin:2plus)`.
- `load()` (the complete-config path) keeps requiring completeness per
  file, so `--config partial.json` fails early with
  `x.json: machine.model is required (is this an overlay? use
  --config-overlay)` — a config and an overlay stay distinct concepts
  even though they share types.
- Aux/model cross-checks (`aux` is //e-only) that need `model` move to
  the completeness pass — an overlay adding `machine.aux` can't be
  judged until the merged document says what the model is.
- **Schema.** `schemars` derives requiredness from `Option`, so the
  relaxed types relax `schema/ewm-config.schema.json` too. Options:
  1. one relaxed schema serving both configs and overlays (simplest;
     editors lose the "model required" hint);
  2. post-process requiredness back into the full schema in the
     generation test and emit a second
     `schema/ewm-config-overlay.schema.json` as derived.
  Recommendation: **(2)** — the golden-file test already owns schema
  generation, adding `"required": ["machine"]` / `["model"]` there is a
  few lines, and overlay files get their own `$schema` for editor
  support. **Decision (as built): (2)** — see `notes/JSON_CONFIG.md`
  "Config sources — partial configs".
- **Gate:** unit tests — a bare `{}` and a slots-only fragment parse and
  round-trip through `load_document`; structural errors still name the
  file; `from_document({})` fails with the completeness message; both
  schemas regenerate and match; full standard gates. Behavior of every
  existing command line is unchanged (the whole suite is the tripwire).

### C3 — `--config-overlay <path>`

- Pass 1 of `parse_options` gains the flag. All document sources —
  `--config`, `--config-overlay`, `--set` — apply **strictly in
  command-line order** through the existing merge; the only differences
  are per-file completeness (C2) and the materialization rule below.
- Overlay files load through the same typed path: structural validation
  with the overlay file named in errors, **relative paths resolved
  against the overlay file's directory** (so
  `--config-overlay drives/total-replay.json` finds `tr.hdv` next to
  itself — the same portability property config files have).
- **Slots materialization extends to overlays.** Today `--set` entering
  `machine:slots` on a slotless document materializes the default table
  first, so the override *extends* the default machine. An overlay
  carrying a `slots` table onto a slotless base must do the same —
  otherwise `ewm two --config-overlay hdd7.json` would produce a literal
  one-slot machine instead of "the default machine plus a hard drive in
  slot 7". Rule: when an overlay's `machine.slots` merges into a
  document without one, materialize `default_slots_value()` first. (A
  *complete* `--config` with an explicit table stays literal, as today.)
- Overlays without any `--config` are legal: like bare `--set`, the
  document starts from the default machine
  (`{"machine": {"model": "2plus"}}`).
- `builtin:` resolution from C1 applies to `--config-overlay` too — the
  source resolver is shared, and it costs nothing. Whether we *ship*
  built-in overlays (e.g. `builtin:vnc`) is a backlog item.
- **Decision for kickoff — multiplicity of `--config`.** Today multiple
  `--config` files deep-merge; with overlays that reads as an accident
  (two "complete machines" silently merging). Recommendation: allow at
  most one `--config`; a second errors with `use --config-overlay for
  additional layers`. Mildly breaking, clearly better semantics — but
  the owner may prefer keeping it as sugar. **Decision (as built): at
  most one `--config`**, per the recommendation — see
  `notes/JSON_CONFIG.md` "Config sources — overlays".
- Usage text: `--config-overlay <path>  layer a partial config on top;
  repeatable, applied in order`.
- **Gate:** `two.rs` option tests — base + overlay + overlay + `--set`
  compose in order; overlay-only extends the default machine; the
  total-replay example works as an integration test (fixture overlay in
  `ewm/tests/configs/`); relative paths resolve against the overlay's
  dir; error cases (overlay with a typo'd key names the overlay file; a
  complete config passed to `--config-overlay` is fine; a partial one
  passed to `--config` is not). Full standard gates.

### C4 — `--print-config`

With four layering sources, "what machine did I just describe?" needs a
first-class answer:

- `ewm two ... --print-config` assembles the document exactly as a real
  run would (configs, overlays, sets, *and* the convenience-flag
  overrides), prints the final merged JSON to stdout, and exits 0.
  Validation errors print as usual and exit nonzero — so it doubles as a
  config linter for scripts and CI.
- Printing after the convenience flags apply means serializing
  `Options` back into a `Config` for the flag-covered fields — the seed
  of JSON_CONFIG Phase C ("save current setup"); keep the mapping in one
  function so Phase C reuses it. If that turns out to drag in too much,
  the fallback (decide at kickoff) is printing the document *before*
  convenience flags with a note, and leaving flag capture to Phase C.
  **Decision (as built): the full mapping** (`options_to_config` in
  `two.rs`) — the fallback wasn't needed; see `notes/JSON_CONFIG.md`
  "Config sources — `--print-config`".
- **Gate:** e2e test — a composed command line prints a document that,
  fed back via `--config`, yields the identical `Options`; full standard
  gates.

### C5 — Docs sweep

- README: rewrite the config section around the four sources with the
  Total Replay overlay as the worked example; `--config builtin:…` in
  the quick-start.
- `notes/JSON_CONFIG.md`: new "Config sources" section recording the
  as-built decisions (builtin naming, one-`--config` ruling, overlay
  materialization rule, schema choice); status table updated.
- `notes/MAC_APP.md` / `notes/IDEAS.md` touch-ups where they mention
  config loading.
- **Gate:** the README examples run verbatim (scripted check); standard
  gates. *(As built: the check is the `readme_two_examples_parse` test
  in `two.rs` — it extracts every `cargo run --release -- two …` example
  from the README and runs it through `parse_options`, which opens every
  `--config`/`--config-overlay` source; the example configs are
  committed under `examples/`.)*

## Hazards

- **The materialization asymmetry** (C3) is the one subtle rule: literal
  slots tables from a base `--config`, extending semantics from overlays
  and `--set`. Get the tests for the four combinations (base with/without
  slots × overlay with/without slots) in place before wiring the flag.
- **Schema lockstep**: C1 (`description`) and C2 (requiredness) both
  touch `schema/ewm-config.schema.json`; regenerate per phase, never
  hand-edit.
- **Error-message drift**: several existing tests pin error strings
  (`test.json: machine.slots: …`); the validation split moves some
  messages between passes — expect test churn in C2, keep the messages
  at least as good.
- **`boo` launcher**: uses `--set` only; unaffected, but re-check its
  drag-drop paths after C3 (the materialization rule must keep behaving
  identically for `--set`).

## Backlog (out of scope, recorded)

- **Default machine as a built-in** — make bare `ewm two` boot
  `builtin:<name>` so code and configs stop describing the default
  machine twice.
- **Built-in overlays** — e.g. `--config-overlay builtin:vnc` for a
  canned headless-serve layer.
- **User config directory** — resolve bare names against
  `~/.config/ewm/configs/` (`--config mysetup`); interacts with the
  `.ewmachine` bundle work (MAC_APP Phase 3).
- **Save current setup** (JSON_CONFIG Phase C) — C4's Options→Config
  mapping is the seed.
- **Config fingerprint in state files** (STATE.md §10) — layered configs
  make hardware-mismatch detection more valuable, not less.
