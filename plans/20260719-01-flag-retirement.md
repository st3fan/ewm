# Flag Retirement: the CLI After Config Sources

- **Design doc:** `notes/JSON_CONFIG.md` (the config document model and
  the four-source surface — `--config`, `--config-overlay`, `--set`,
  convenience flags — this plan is the follow-through: now that every
  machine setting lives in the document, most convenience flags are
  duplicates and can go).
- **Status:** draft — iterate here before kickoff
- **Target:** `main`; one PR per phase unless decided otherwise at kickoff

## Goal

Shrink `ewm two`'s command line to the source surface plus debug
tooling. After this plan:

```
ewm two [--config <source>] [--config-overlay <source>]... [--set k=v]...
        [--print-config] [--serve <url>] [--wozbug [port]] [--break <addrs>]
```

Everything else is said through the document. The payoff is more than a
shorter usage screen: pass 2 of `parse_options` collapses to a handful
of tool flags, the "flags override the finished document" precedence
machinery mostly disappears, and the machine has **one** construction
path — document → `from_document` → `apply_config`.

## The inventory (`ewm two`)

Every flag `parse_options` accepts today, and the verdict. "Retire"
means the flag is removed outright — it falls into the generic
`usage()` error like any unknown flag. The app is v0.0.1; no
transition ceremony.

### Stays — the source surface

| Flag | Why it stays |
|---|---|
| `--config <source>` | the document base |
| `--config-overlay <source>` | partial layers |
| `--set <key>=<value>` | single-value overrides |
| `--print-config` | inspection / linting |
| `--help` | obviously |

### Stays — debug tooling, deliberately not in the config (C4 ruling)

| Flag | Why it stays |
|---|---|
| `--wozbug [port]` | debugger server; session tooling, not machine description |
| `--break <addr,..>` | breakpoints; same |
| `--screenshot=<path>` | hidden test-harness flag; the golden-BMP tripwire |

### Stays — structured sugar (recommended; owner may overrule)

| Flag | Config equivalent | Why keep it |
|---|---|---|
| `--serve <url>` | `remote.*` | Not a single-value duplicate: one URL carries bind, port, ws/web, password, view-only. Retiring it means five `--set`s or a file for the common headless case. Revisit once **built-in overlays** (config-sources backlog, e.g. `--config-overlay builtin:vnc`) exist — but note an overlay still can't carry a per-run password as ergonomically as the URL does. |

### Retire — single-value duplicates of config keys

| Flag | Config key | Replacement | Notes |
|---|---|---|---|
| `--color [style]` | `display.monitor` | `--set display:monitor=rgb` (etc.) | The oldest muscle-memory flag; bare `--color` = rgb. Its optional-value "peek" parsing goes with it. **Owner sign-off (F2).** |
| `--scanlines [level]` | `display.scanlines` | `--set display:scanlines=light` | Same optional-value pattern. |
| `--fps <n>` | `display.fps` | `--set display:fps=60` | |
| `--strict` | `cpu.strict` | `--set cpu:strict=true` | |
| `--debug` | `debug.enabled` | `--set debug:enabled=true` | |
| `--boot-delay <s>` | `boot.delay` | `--set boot:delay=3` | debugging/recording aid |
| `--trace [<file>]` / `--trace=<f>` | `debug.trace` | `--set debug:trace=/dev/stderr` | bare `--trace` meant `/dev/stderr`; the replacement spells it out |
| `--state <path>` | `state.path` | `--set state:path=game.state` | Recent, deliberate UX (STATE S0/S4) — counterargument recorded; **owner decision.** |
| `--model <2plus\|2e>` | `machine.model` | `--config builtin:2e`, or `--set machine:model=2e` | Alias spellings (`2+`, `][+`, `//e`, `iie`) disappear. Most-used flag; **owner sign-off (F2).** |
| `--aux <card>` | `machine.aux` | `--set machine:aux:card=ramworksiii --set machine:aux:size=1m`, or a config | The `--aux` token stays *internally* (`Options.aux`, rebuilt by `apply_config`); only the flag goes. |

### Retire — with an interlock

| Flag | Config key | Interlock |
|---|---|---|
| `--memory <region>` | `machine.memory` | `apply_set` rejects array paths — its error literally says "memory regions come from `--memory`". Retiring the flag makes overlay/config files the *only* memory-region path. Either accept that (recommended: it is an obscure power feature, and a two-line overlay file is fine) or teach `--set` array append first. Update the `apply_set` error text either way. |

## `ewm one`

`one` has four flags — `--model <apple1|replica1>`, `--memory`,
`--trace`, `--strict` — and **no JSON config support at all**
(`config.rs` is `two`-shaped: `Model` is `2plus`/`2e`). Nothing has
"moved into config files" for `one`, so there is nothing to retire.

**Recommendation: leave `one` alone; out of scope.** Porting a config
surface to `one` only pays for itself bundled with REMOTE.md Phase 7
(serving `one` over VNC), which will want a `remote.*` block anyway —
record "minimal `one` config (model, memory, debug) + flag retirement"
as a backlog item there, don't block `two`'s cleanup on it.

## Known flag consumers to sweep

- `main.rs` top-level usage hint: suggests `two --color --set …` — uses
  a retired flag.
- `README.md` quick-start and WozBug examples (`--color`, `--model`,
  `--aux`, `--break`) — the `readme_two_examples_parse` test (C5) fails
  the suite on any stale example, so the sweep is enforced, not hoped.
- `boo` launcher and drag-drop: already `--set`-only (verified) — no
  change.
- Notes: `REMOTE.md` (uses `--serve` — staying), `STATE.md` (uses
  `--state` — sweep if retired), `DEBUGGING_TOOLS.md` (`--break`,
  `--wozbug` — staying).
- In-crate `two.rs` option tests: the tests *for* retired flags are
  deleted with them; composition tests move to `--set` spellings.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| F1 | Retire the quiet seven: `--scanlines`, `--fps`, `--strict`, `--debug`, `--boot-delay`, `--trace`, `--state`(*) — tests, usage, docs | M | Not started |
| F2 | Retire the muscle-memory trio: `--model`, `--color`, `--aux` — README quick-start rewritten builtin-first, `main.rs` hint updated | M | Not started |
| F3 | Retire `--memory`; `apply_set` error text updated; overlay documented as the memory-region path | S | Not started |

(*) `--state` moves to F1 only if the kickoff decision says retire.

F1 before F2 deliberately: the quiet flags find the process problems
(error-message shape, test churn, docs sweep) before the high-traffic
flags go. F3 is independent of F2 but reads best last.

- **Gate (every phase):** standard gates (`fmt`, `clippy -D warnings`,
  full `cargo test` incl. golden-BMP); `readme_two_examples_parse`
  green; `--print-config` round-trip tests still pass (they compose via
  `--set` after F1/F2 rewrites).

## Decisions to make at kickoff

1. **F2 scope** — retire all three of `--model`/`--color`/`--aux`, or
   keep any as permanent sugar? (Recommendation: retire all three;
   `--config builtin:2e` is as short as `--model 2e` and better.)
2. **`--state`** — retire (consistency) or keep (recent deliberate UX)?
   Recommendation: retire; `--set state:path=…` is one token longer.
3. **`--serve`** — confirmed keep, or schedule retirement behind
   built-in overlays? Recommendation: keep.
4. **`--memory`** — overlay-only, or extend `--set` with array append
   first? Recommendation: overlay-only.
5. **PR granularity** — per phase (default) or one PR.

## Hazards

- **The optional-value "peek" parsing** (`--color [style]`,
  `--scanlines [level]`, `--wozbug [port]`) is subtle and pinned by
  tests; deleting the first two must not disturb `--wozbug`'s.
- **`options_to_config` is unaffected** (it maps `Options`, not flags),
  but its round-trip tests compose command lines with retired flags —
  rewrite them to `--set` spellings in the same phase, not after.

## Backlog (recorded, out of scope)

- **`one` config surface** — minimal config (model, memory, debug) for
  `one`, bundled with REMOTE.md Phase 7; retire `one`'s flags then.
- **Built-in overlays** (`builtin:vnc`) — prerequisite for ever
  revisiting `--serve`.
- **`--set` array append** — only if overlay-only memory regions turn
  out to chafe.
