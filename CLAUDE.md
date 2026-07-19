# EWM — Project Instructions

## How we work: ideas → plans → pull requests

All non-trivial work is plan-driven. The flow:

1. **Idea / design** — substantial ideas get a working document in
   `notes/` (e.g. `notes/STATE.md`, `notes/REMOTE.md`): architecture,
   trade-offs, alternatives considered, hazards. Notes are living documents
   — re-read them at the start of a session, update them as reality
   diverges (*as built* notes), and keep their status tables current.
2. **Plan** — before implementation starts, write a **phased plan** in
   `plans/`, named **`YYYYMMDD-NN-slug.md`** (date the plan was created,
   `NN` = two-digit sequence within that day, short kebab-case slug —
   e.g. `plans/20260718-01-machine-state.md`). The plan is the execution
   roadmap: it references the design note for rationale rather than
   repeating it, and defines phases.
3. **Pull requests** — work proceeds from the plan: either **one PR per
   phase** (the default) or the whole plan in one PR — **the user decides
   which, per plan; ask at kickoff, don't assume.** Keep the plan's phase
   table updated as PRs land.

**All work goes through a pull request — no direct commits to `main`**
(enforced with branch protection, so a push to `main` will be rejected
anyway). This applies to everything, including one-line doc fixes:
branch, push, open a PR, and let the owner merge.

A good phase is independently landable and reviewable, sized (S/M/L), with
an explicit **gate**: what proves it works (tests, a scripted check, a
manual observation). Sequence phases so the tree is never broken in
between — e.g. land a trait before flipping a supertrait bound that forces
impls everywhere.

## Gates (every phase, every PR)

- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` — including the golden-BMP screenshot tests, which are the
  tripwire proving the SDL frontend's behaviour is untouched
- Plus the phase's own feature gate from the plan.

## The configuration surface

- **New machine settings go in the config document, never in new CLI
  flags.** The serde types in `ewm/src/config.rs` are the source of
  truth; every key is reachable via `--config` / `--config-overlay` /
  `--set` and inspectable with `--print-config`. Both subcommands'
  CLIs are exactly the sources plus debug tooling (`--wozbug`,
  `--break`, `--serve`, hidden `--screenshot=`); the per-setting flags
  were retired deliberately (`plans/20260719-01`) and don't come back.
  Design and as-built decisions live in `notes/JSON_CONFIG.md`.
- **One document type describes any machine.** `machine.model` decides
  the *family* (apple2 → `ewm two`, apple1 → `ewm one`); keys that
  don't apply to a family are rejected by name in
  `config::validate_complete` — the single place to relax when a
  feature reaches the other family.
- **The JSON Schemas are generated, never hand-edited.**
  `EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed`
  regenerates `schema/ewm-config.schema.json` and
  `schema/ewm-config-overlay.schema.json` together.
- **README examples are executable-checked**: `readme_examples_parse`
  parses every `cargo run --release -- two|one …` example through the
  real option parsers, and the example configs they reference are
  committed under `examples/`. Edit examples accordingly — a stale one
  fails the suite.
- **Pre-1.0 CLI ruling (owner's):** removed flags go outright — no
  deprecation cycle, no transition error messages.

## House style (short version)

- **Dependency budget is tiny and deliberate** — prefer hand-rolling on
  `std` (the codebase implements its own RFB, WebSocket, DES, SHA-1, BMP,
  PNG, chunk formats) and test hand-rolled primitives against published
  RFC/FIPS vectors. Propose a crate only when hand-rolling is clearly
  unreasonable, and say so in the plan.
- Blocking I/O and threads (`std::net` + `mpsc`, the `wozbug::Server`
  shape) — no async runtime.
- Doc comments explain *why* and point at the relevant note
  (`notes/FOO.md §n`); commit messages tell the story of what was verified.
