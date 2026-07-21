# Real AppleMouse hardware (6821 PIA) — the //e-firmware path

- **Design docs:** self-contained (below), building on the synthetic card's
  as-built `notes/MOUSE.md` (and its "Known limitation" section — the
  diagnosis this plan answers). Background: `notes/JSON_CONFIG.md` (slots),
  `notes/STATE.md` (`Persist`), `plans/20260721-01-apple-mouse-card.md` (the
  synthetic card this replaces on the wire). Primary source: the *Apple II
  Mouse Technical Note*, the AppleMouse II schematic (the 6821 PIA wiring),
  and the //e's internal mouse firmware in ROM.
- **Status:** planned — not started.
- **Target:** `main`; PR granularity decided at kickoff (one PR per phase is
  the default).

## Why (the diagnosis)

The synthetic-firmware card (`plans/20260721-01`) works for software that
calls the card's **own** `$Cn00` entry points — but not for MousePaint / GEOS
/ Dazzle Draw on the **//e**. Verified 2026-07-21 by booting the real
MousePaint disk headlessly:

- MousePaint reaches its menu ("Press RETURN … or click the mouse button")
  and **hangs** — a scripted RETURN changes nothing.
- On the //e it sets `INTCXROM=1` and runs the mouse code in the //e's
  **internal `$Cx` ROM**, which drives the card as its **real 6821 PIA**
  (`$C0n0-$C0n3`), *not* the `$Cn00` entry points. Our `Mou` presents a
  synthetic DEVSEL protocol — so the //e firmware's init fails (it touched the
  card twice, then gave up), and the menu loop (gated on the mouse before it
  polls the keyboard) never proceeds.

The card must be modelled as its **real hardware** so the //e's own firmware
drives it.

## The design

Model the AppleMouse II as what it is: a **6821 PIA** at the slot's DEVSEL
range (`$C0n0-$C0n3`), with the mouse mechanism — **X/Y quadrature encoders**,
the **button**, and the **interrupt lines** — wired to the PIA's ports exactly
as the real card wires them. Both the //e's internal firmware and (Phase P5)
our own `$Cn00` firmware then drive the *same* faithful hardware.

- **Infra:** `ewm/src/pia.rs` is already a 6820/6821 PIA (the Apple 1's
  keyboard/display PIA). Reuse or generalize it — this is the substrate.
- **The mouse → PIA wiring** is the crux and the risk: which PIA port bits
  carry X movement / X direction, Y movement / Y direction, the button, and
  how the interrupt (CA1/CB1) is configured. Reverse-engineered in P1 from the
  //e firmware's real accesses plus the schematic.
- **Host input** (the M3 feed) drives the quadrature counters / button instead
  of the synthetic device's position registers.
- **//e:** the internal firmware drives the PIA — works once the wiring is
  faithful; no card ROM / `$C800` needed (the //e *is* the firmware).
- **][+:** no internal mouse firmware, so the card needs its own ROM. Keep the
  eight-entry `$Cn00` firmware but **rewrite `mouse_rom` to drive the PIA**
  (a compact PIA-reading firmware) rather than the retired synthetic protocol
  — so the ][+ keeps working without the un-modelled `$C800` expansion.

This **replaces** the synthetic DEVSEL protocol (`plans/20260721-01` M2): the
DEVSEL device becomes the PIA, and the M2–M4 tests that assert the synthetic
protocol are migrated (P6) as their subject changes.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| P1 | Reverse-engineer + document the AppleMouse PIA wiring and the //e firmware's protocol; a headless MousePaint-to-menu dev harness | M | Planned |
| P2 | The 6821 PIA + mouse-mechanism device (quadrature/button/IRQ lines) at the DEVSEL; host input feeds it | M | Planned |
| P3 | //e MousePaint boots to a working pointer (polled); the flagship gate | M/L | Planned |
| P4 | Interrupts via the PIA (CA1/CB1 → the M1 line); interrupt-driven //e software | M | Planned |
| P5 | The ][+ path: `mouse_rom` rewritten to drive the PIA; migrate the M2 firmware gate | M | Planned |
| P6 | Docs + as-built `notes/MOUSE.md`; retire/migrate the synthetic-protocol tests | S | Planned |

Order **P1 → P6**. P1 is the load-bearing phase — everything rests on the
wiring being right. Every phase runs the standard gates (`cargo fmt --all
--check`, `cargo clippy --all-targets -- -D warnings`, full `cargo test`
**including the golden-BMP screenshots** — no mouse in those configs, so they
stay the tripwire) plus the phase gate.

### P1 — reverse-engineer the PIA protocol

- A dev harness that builds a //e with a slot-4 mouse + the MousePaint disk
  (downloaded at dev time — not committed, see hazards), boots it, and logs
  every `$C0n0-$C0n3` access with value + PC. From that plus the schematic,
  document: the PIA register use (DDRA/CRA/PA, DDRB/CRB/PB), which bits are
  X-count / X-dir / Y-count / Y-dir / button, and the interrupt-line
  configuration.
- **Gate:** the protocol written up in `notes/MOUSE.md` (an "as-built
  hardware" section), backed by the captured trace. No CI gate here beyond the
  standard ones (the harness is a dev tool; the disk is not redistributable).

### P2 — the PIA + mouse mechanism

- The `Mou` device becomes a 6821 PIA (reusing `pia.rs`) with the mouse
  mechanism wired to its ports per P1: quadrature X/Y counters advanced by
  host movement, a button bit, and CA1/CB1 interrupt inputs.
- Host input (`feed_mouse_*`) drives the quadrature/button instead of the
  synthetic position registers.
- **Gate:** unit tests — a fed movement/button appears at the PIA ports the
  way the firmware reads them (drive the PIA registers directly, à la the M2
  device tests).

### P3 — //e MousePaint works (polled)

- With the faithful PIA, the //e internal firmware's ReadMouse/SetMouse work;
  MousePaint boots to a live pointer.
- **Gate — flagship:** headless, deterministic. Boot MousePaint (dev-harness
  disk), script a pointer interaction, and assert the mouse position the //e
  firmware reports (the screen holes) tracks it — a firmware-level assertion,
  not a committed golden-BMP (the image is not redistributable; recorded in
  `notes/MOUSE.md`). A committable synthetic //e-firmware-driving test is the
  CI gate.

### P4 — interrupts via the PIA

- The PIA's interrupt (CA1/CB1) asserts the M1 IRQ line; the //e firmware's
  interrupt-driven mode delivers VBL/movement/button through the //e ROM's IRQ
  handler (distinct from the ][+ `$03FE` path P… M4 used — the //e handler
  saves MMU state first).
- **Gate:** an interrupt end-to-end test on the //e.

### P5 — the ][+ path

- Rewrite `mouse_rom` so the `$Cn00` firmware drives the PIA (read the
  quadrature, integrate, clamp, populate the screen holes) — the ][+ keeps a
  working mouse via the card's own firmware, no `$C800`.
- **Gate:** the migrated M2 scripted firmware test (now PIA-driven) +
  card-firmware software.

### P6 — docs + as-built

- `notes/MOUSE.md` rewritten as the hardware as-built; the golden `mouse_rom`
  test and the synthetic-protocol device tests migrated or retired; README
  unchanged (the config surface is the same `{"card":"mouse"}`).
- **Gate:** standard gates.

## Hazards

- **The wiring is the whole game.** If P1 gets the PIA port bits or interrupt
  config wrong, the firmware mis-reads and nothing works — pin it with real
  traces, not guesses.
- **The MousePaint disk is not redistributable** — the flagship gate cannot
  commit the image. Use a dev-time-downloaded harness for bring-up and a
  committable firmware-level deterministic assertion for CI; record the choice.
- **This replaces the synthetic DEVSEL protocol** — `plans/20260721-01`'s M2
  device tests and `mouse_rom` change subject; migrate them (P5/P6), don't
  leave two contradictory DEVSEL meanings.
- **//e IRQ delivery** differs from the ][+ path the last plan tested: the //e
  ROM IRQ handler saves/restores INTCXROM + MMU state before reaching the user
  routine. P4 must exercise the real //e handler.
- **Timing / quadrature rate** — the firmware integrates quadrature pulses; a
  wrong pulse rate makes the pointer move too fast/slow. Tune against
  MousePaint feel in P3, pin the mapping in a test.

## Decisions to make at kickoff

1. **][+ path** — rewrite `mouse_rom` to drive the PIA (recommended; keeps the
   ][+ mouse) vs. a real card ROM + `$C800` expansion (bigger, models the real
   card fully) vs. drop the ][+ mouse (the //e is MousePaint's target anyway).
2. **Flagship gate** — a committable synthetic //e-firmware-driven assertion
   (recommended, CI-safe) vs. a golden-BMP that needs the non-redistributable
   disk (dev-only).
3. **PR granularity** — one per phase (default) or fewer.

## Backlog (recorded, out of scope)

- The real AppleMouse `$Cn00` + `$C800` ROM (full card fidelity for the ][+),
  which needs the per-slot `$C800` expansion EWM omits.
- Mockingboard on the shared IRQ line; the `//c` built-in mouse (same PIA
  substrate, wired to the IOU).
