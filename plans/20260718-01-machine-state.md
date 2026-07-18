# Machine State Save/Restore (`--state`)

- **Design doc:** `notes/STATE.md` (architecture, trait design, format,
  component inventory, hazards — read it first; this plan is the execution
  roadmap and does not repeat the rationale)
- **Status:** implemented (PR pending); see *as built* notes in notes/STATE.md
- **Target:** `main`, one PR per phase unless decided otherwise at kickoff

Start the emulator with `--state=/some/mystate`: restore that state at
startup when the file exists, save it back at quit — suspend/resume for an
Apple II. v1 assumes the same hardware configuration across runs (mismatch
detection is backlog, `notes/STATE.md` §10).

Every phase lands green: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`, golden-BMP tests
untouched. Update the phase table below as phases land; record deviations as
*as built* notes in `notes/STATE.md`.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| S0 | Plan + CLI/config surface | S | Done (CLI landed inside S4 — see as-built note) |
| S1 | `state.rs` chunk container | S | Done |
| S2 | `Persist` trait + Cpu/Memory | M | Done |
| S3 | The supertrait flip: all devices | L | Done |
| S4 | Lifecycle wiring | M | Done |
| S5 | Determinism gate + e2e | S | Done |

### S0 — Plan + CLI/config surface

- This plan and `notes/STATE.md` (done).
- `--state <path>` on `ewm two` and a `state.path` config field
  (schema-regenerated, path resolved relative to the config file, CLI
  overrides config — the `remote` block is the pattern to copy).
- Until S4, a configured state path errors "state persistence not built
  yet (S4)" at startup rather than being silently ignored.
- **Gate:** parse/override unit tests; schema test green.

### S1 — `state.rs` chunk container

- New `ewm-core/src/state.rs`: `Writer`/`Reader` with typed LE helpers,
  nested tag+length chunks, `EWMS` magic + version, `Error` type; atomic
  file save (temp + rename). No dependencies, no panics on corrupt input.
- **Gate:** unit vectors — round-trips, nesting, truncation, bad
  magic/version, every typed accessor.

### S2 — `Persist` trait + structural state

- `Persist` in `ewm-core` (final name bikeshed here): `save(&self, &mut
  Writer)` / `restore(&mut self, &mut Reader) -> Result`.
- Impls for `Cpu` (registers, counter) and `Memory` (`base_ram`, `Ram`
  regions by index; ROM/watchpoints skipped). **No `Device: Persist` bound
  yet** — device chunks deferred to S3 so this phase stays reviewable.
- **Gate:** round-trip test on a cardless machine: save → restore into a
  fresh twin → registers and RAM identical.

### S3 — The supertrait flip (one mechanical PR, all devices at once)

- `Device: Any + Persist` and `AuxCard: Persist`; `Memory::save/restore`
  gains the index-tagged device chunks.
- Impls: `TwoIo`, `IouE` (delegating to its aux card), all `AuxCard`s
  (Ext80Col, RamWorks), `Alc`, `Saturn`, `Dsk` (drives, arm, motor +
  spin-down stamp, latch, **media contents + dirty flags** — floppy writes
  are memory-only), `Hdd`/`Liron` (controller state only; media lives in
  the backing files), `Clk` (explicitly empty). Cycle stamps saved
  verbatim, never rebased.
- **Gate:** per-component round-trips; full-machine round-trip with a
  booted DOS 3.3 (text screen + media bytes identical).

### S4 — Lifecycle wiring

- Startup: restore-instead-of-reset when the file exists; fresh boot when
  absent; restore failure is fatal with a clear message.
- Quit: SDL quit event saves; headless serve loop saves on SIGINT/SIGTERM
  (raw `unsafe extern "C"` libc declarations, handler sets an
  `AtomicBool` — no new dependency). Save failure exits nonzero leaving
  the previous file intact.
- S0's "not built yet" error replaced by the real thing; usage text and
  `notes/STATE.md` updated.
- **Gate:** manual: boot, poke memory, quit, restart, observe it survived —
  both frontends.

### S5 — Determinism gate + end-to-end test

- The automated version of S4's manual gate: boot N frames → save →
  restore into a fresh machine → framebuffer/`text_screen()` identical;
  run both K further frames → still identical.
- CLI e2e: `X=42`, quit, restart, `PRINT X` → `42`.
- `notes/REMOTE.md` note: serve-mode suspend/resume for the VM farm.
- **Gate:** the new tests themselves, green in CI.

## Backlog

See `notes/STATE.md` §10 — config fingerprint, forward compatibility,
`one` support, save-while-running, named snapshots, compression, floppy
write-back as its own feature, WozBug state-file inspection.
