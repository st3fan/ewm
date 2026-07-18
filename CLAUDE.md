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
