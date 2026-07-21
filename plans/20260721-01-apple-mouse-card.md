# The AppleMouse II Card (`{"card": "mouse"}`, slot 4)

- **Design docs:** this plan carries its own design (firmware model,
  interrupt model, coordinate model) — self-contained like the
  original-][ and telnet plans. Background: `notes/JSON_CONFIG.md` (the
  slot/config machinery), `notes/REWRITE.md` (how a card is wired into
  `Two`), `notes/STATE.md` (the `Persist` contract a new device owes),
  `notes/REMOTE.md` §7 (RFB input). The as-built note this plan will
  produce is `notes/MOUSE.md` (Phase M5). Primary source: the *Apple II
  Mouse Technical Note* / *AppleMouse II User's Manual* and the //e Mouse
  firmware — the eight-entry firmware protocol and its screen-hole
  contract.
- **Backlog origin:** `notes/IDEAS.md` → "AppleMouse card (M) — unlocks
  MousePaint, Dazzle Draw menus, GEOS."
- **Status:** in progress — M1 (interrupt path) and M2 (the mouse card
  substrate, polled) landed; M3–M5 remain.
- **Target:** `main`; **one PR per phase** (owner directed).
- **Kickoff decisions (this build):** (1) coordinates — **relative/captured**;
  (2) clamp — **firmware default `0..=1023`, no `mouse` config fields**;
  (3) M4 flagship — the **scripted firmware-level end-to-end test** (no
  redistributable disk image committed); (4) IRQ line — **`Two`-local**,
  polled by the burst loops.

## Goal

An Apple II Mouse in a peripheral slot, configured like every other card
(no new CLI flags — the config surface rule):

```json
{
  "machine": { "model": "apple2e", "slots": { "4": { "card": "mouse" } } }
}
```

```
ewm two --config myiie-mouse.json      # or --set 'machine:slots:4:card=mouse'
```

With it, MousePaint boots to a working pointer, Dazzle Draw / Blazing
Paddles menus are drivable, and GEOS sees a mouse. The host mouse (SDL
window, or an RFB/VNC client's pointer) drives the emulated one; the
firmware's eight entry points behave per the Technical Note, and a
program that enables VBL interrupts gets them.

The card is **apple2-family, both ][+ and //e** (unlike `aux`, which is
//e-only, or slot 0, which is ][+-only). Slot 4 is the canonical home
(what MousePaint and GEOS probe first), but any slot 1–7 is allowed.

## The card: synthetic firmware, not the real ROM

We follow the house pattern the Thunderclock and hard-drive cards already
use (`ewm/src/clk.rs`, `ewm/src/hdd.rs`): **hand-assembled firmware that
implements the documented protocol**, backed by a Rust `Device` in the
slot's DEVSEL range — *not* a simulation of the card's 6821 PIA + 68705
microcontroller.

Why synthetic:

- **It fits the 256-byte `$Cn00` page.** The real AppleMouse firmware
  spills into the `$C800-$CFFF` expansion ROM, and EWM has **no per-slot
  `$C800` expansion** — `two.rs` states it outright: "no peripheral card
  in EWM has a `$C800` expansion ROM … per-slot expansion is out of
  scope" (the internal //e slot-3 firmware is the only `$C800` user). A
  real ROM would drag in that whole latch mechanism; synthetic firmware
  that lives entirely in `$Cn00` sidesteps it, exactly as `clk_rom` does.
- **The firmware protocol *is* the sanctioned interface.** MousePaint,
  GEOS, Dazzle Draw and the ProDOS `MOUSE.OBJ` driver all call the eight
  documented entry points; almost nothing pokes the PIA directly. Emulate
  the contract, and the software is satisfied (this is izapple2's
  approach too).

### Firmware layout (`mouse_rom(slot)`, mirroring `clk_rom(slot)`)

A per-slot 256-byte generator with the slot-dependent operands patched, a
golden byte-for-byte test pinning slot 4 (as `clk.rs` pins slot 1).

- **Identification bytes** (Pascal 1.1 firmware protocol + mouse ID):
  `$Cn05=$38`, `$Cn07=$18`, `$Cn0B=$01` (implements the protocol),
  `$Cn0C=$20` (X-Y pointing device), `$CnFB=$D6` (AppleMouse). And
  `$Cn01 ≠ $20`, so the Autostart slot scan never mistakes the card for a
  bootable Disk II — the same trap `clk.rs` documents.
- **Entry-offset table** at `$Cn12-$Cn19`: one byte each giving the low
  byte of the routine's address within page `$Cn`, in the fixed order
  SetMouse, ServeMouse, ReadMouse, ClearMouse, PosMouse, ClampMouse,
  HomeMouse, InitMouse.
- **The eight routines**, which move state between the caller's
  **screen holes** and the card's DEVSEL soft switches:
  - `ReadMouse` latches the device, then deposits X (lo/hi), Y (lo/hi),
    the status byte (button now / button last / moved), and the mode into
    the per-slot screen holes (`$0478+n`, `$04F8+n`, `$0578+n`,
    `$05F8+n`, `$0778+n`, `$07F8+n` — exact map per the Technical Note).
  - `SetMouse` writes the mode byte (bit0 on = mouse enabled; bits 1-3 =
    interrupt on movement / button / VBL).
  - `ClampMouse` sets the X-or-Y clamp bounds; `HomeMouse` sets position
    to the clamp minimum; `PosMouse` forces a position; `ClearMouse`
    zeroes position; `ServeMouse` reports & clears the interrupt source;
    `InitMouse` resets to defaults (clamp `0..=1023`, mouse off).

### The device (`ewm/src/mouse.rs`: `Mou`)

A `Device` (DEVSEL soft switches) + `Persist`, holding: 16-bit X/Y,
clamp bounds `min/max` per axis, button state (now + last-read), a
"moved since last read" flag, the mode byte, and the pending
interrupt-source bits. The soft switches are the private wire between
`mouse_rom`'s routines and this struct — only our firmware reads them, so
the exact port assignment is ours to choose (documented in `notes/MOUSE.md`).

## The interrupt model (the real prerequisite)

The mouse card raises a maskable **IRQ** on VBL, movement, or button per
the SetMouse mode; MousePaint and GEOS install a handler that calls
ServeMouse + ReadMouse each VBL. **EWM has no IRQ delivery today**:
`cpu.irq()` exists but nothing asserts a line into it, and RDVBL
(`$C019`) is *faked* from the cycle counter ("not cycle-modelled — quirk
#3"). So the first phase builds a real, reusable maskable-interrupt path
(Mockingboard will want the identical line — `notes/IDEAS.md`).

Design (Phase M1, detailed below): a **level-sensitive IRQ line** the
machine caches as a single `bool`, recomputed only when an
interrupt-capable device changes state (so the per-instruction check in
the burst loop is a `bool` read + the CPU `I` flag — no per-instruction
device scan, which would cost at ~1 MHz). The burst loops
(`two.rs::run`, the headless `serve_rfb`, the web loop) already run in
fixed steps; between `cpu.step()` calls, if the line is high and `I==0`,
service it. The VBL source ticks **once per frame** (60 Hz), matching the
frame loop — deterministic, which the golden-BMP culture requires.
`cpu.irq()` is corrected in the same phase: a hardware IRQ pushes the
*current* PC (not the `+1` BRK hack it does now) with **B clear**, sets
`I`, and vectors through `$FFFE`.

## The coordinate / input model

The host pointer must drive the emulated mouse within its clamp window
(default `0..=1023`). Two faithful options, a **kickoff decision**:

- **Relative (captured)** — SDL relative-mouse mode; integrate host
  deltas and clamp. This is what the hardware does (a mouse reports
  movement, the firmware integrates and clamps), and it sidesteps any
  window-pixel↔clamp-range mismatch. Cost: the cursor is grabbed while
  the window is focused (a grab/ungrab gesture, like other emulators).
- **Absolute (mapped)** — map the window pixel position into the clamp
  range each frame. The host cursor stays visible and uncaptured
  (friendlier for a windowed emulator), at the cost of being less like
  real hardware and needing a live clamp→window mapping.

Recommendation: **relative/captured** for faithfulness, with a
palette/gesture to release the grab. Input is fed **once per frame before
the burst**, exactly as the joystick is re-fed every frame today
(`set_joystick`). The RFB path is nearly free: `rfb.rs` already parses
`PointerEvent { mask, x, y }`; the serve loop currently uses only its
button (maps it to paddle-0). When a mouse card is present, its x/y/mask
feed the mouse device instead.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| M1 | Maskable-interrupt path: a cached machine IRQ line the burst loops poll; corrected `cpu.irq()` (real-IRQ vs BRK semantics) | M | Done |
| M2 | The mouse card substrate: `config::SlotCard::Mouse` + `SlotDevice::Mouse`, the `Mou` device, `mouse_rom(slot)`; polled semantics, headless firmware gate | M | Done |
| M3 | Host input: SDL mouse events + RFB pointer x/y feed the device each frame; capture/grab; the cursor tracks in polled software | S/M | Planned |
| M4 | Interrupt mode: VBL / movement / button assert the M1 line; ServeMouse clears; MousePaint flagship gate | M | Planned |
| M5 | Docs + as-built `notes/MOUSE.md`; README example; schema inventory; tick `IDEAS.md` | S | Planned |

Order: **M1 → M2 → M3 → M4 → M5**. M1 and M2 are independent (M2's polled
mouse has no interrupts); M1 is a hard prerequisite only for M4. Sequence
so the tree is never broken between phases: M1 lands the line + a test
device; M2 lands a card that works when polled; M3 makes it move; M4
flips on interrupts; M5 documents. Every phase runs the standard gates
(`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
full `cargo test` **including the golden-BMP screenshots** — the tripwire
that mouse wiring left existing rendering untouched) plus the phase gate.

### M1 — Maskable-interrupt path

- A machine-level IRQ line: interrupt-capable devices expose their
  asserted state; `Two` caches the OR as one `bool`, refreshed when a
  contributor changes (per frame / on the relevant soft-switch access),
  never scanned per instruction.
- The burst loops check, between `cpu.step()`s: line high **and** `I==0`
  → `cpu.irq()`. Level-sensitive: it stays high (re-taken after `RTI`
  clears `I`) until the device de-asserts.
- Fix `cpu.irq()`: push the current PC with **B clear** (a hardware IRQ,
  not BRK), set `I`, vector `$FFFE`. Keep BRK's existing behavior
  distinct.
- **Gate:** an `ewm-core` (or `ewm`) unit test with a tiny test device
  that asserts IRQ: with `I` clear the CPU vectors through `$FFFE`, runs a
  handler, `RTI`s back to the interrupted PC; with `SEI` the request is
  held pending until `CLI`; the pushed status has **B=0** for IRQ vs
  **B=1** for BRK. No existing test (Dormann, golden-BMP, boots) moves —
  the line is dormant until a device uses it.

### M2 — The mouse card substrate

- **Config** (`config.rs`): `SlotCard::Mouse` (no required fields;
  optional `coords`/clamp deferred to a kickoff decision). Extend
  `card_name()`, `referenced_files` (a mouse references no files),
  `resolve_paths` (nothing to resolve), and the schema — regenerate both
  with `EWM_UPDATE_SCHEMA=1 cargo test -p ewm schema_matches_committed`.
- **Validation** (`validate` / `validate_complete`): mouse is legal in
  slots 1–7 (not slot 0 — no `$Cn00` space there), apple2-family only
  (the apple1 family already rejects `machine.slots` wholesale), and a
  multiplicity cap of **one mouse** (join the `count(...)` checks beside
  the Thunderclock rule). No //e sub-gate — the card fits both ][+ and //e.
- **Machine** (`two.rs`): a `SlotDevice::Mouse` variant wired in
  `new_2plus` / `new_apple2` / `new_2e` (`add_device` for the DEVSEL
  range, `add_rom(slot_rom_base(slot), mouse_rom(slot))`), and the
  `build_machine` `SlotCard::Mouse → SlotDevice::Mouse` mapping.
- **Firmware + device**: `mouse_rom(slot)` and `Mou` per the design
  above; polled semantics only (SetMouse without interrupt bits;
  ReadMouse / ClampMouse / HomeMouse / PosMouse / ClearMouse /
  InitMouse). A position setter for tests to inject movement without a
  frontend.
- **Gate:** a `two_mouse.rs` integration test (mirroring `two_clk.rs`):
  the card is detectable by its ID bytes; 6502 code (or direct soft-switch
  drive) that InitMouse → ClampMouse → PosMouse → ReadMouse deposits the
  expected clamped X/Y/status/mode in the screen holes; clamping is
  enforced at the bounds. A golden `mouse_rom(4)` byte test and the
  moved-slot operand test (both like `clk.rs`). Config unit tests
  (slot/family/multiplicity) + the schema golden.

### M3 — Host input

- **SDL** (`two.rs::run`): handle `Event::MouseMotion` /
  `MouseButtonDown` / `MouseButtonUp`; accumulate into a per-frame
  position/button feed applied before the burst (the `set_joystick`
  cadence). Grab/relative-mode per the coordinate decision, with a
  release gesture (and a palette entry if warranted).
- **RFB** (`two.rs` serve path): when a mouse card is present, route
  `InputEvent::Pointer { mask, x, y }` to the mouse device (x/y mapped to
  the clamp window; `mask` bit0 = button) instead of the current
  paddle-0-button repurposing. The web loop follows the same feed.
- **Gate:** a headless test that feeds synthetic host movement/button
  and confirms a polled `ReadMouse` tracks it and clamps at the window;
  an RFB test (extend the `key_and_pointer_events_reach_the_emulator`
  shape) that a pointer event moves the emulated mouse. Golden-BMP
  screenshots unchanged (no mouse card in those configs → no cursor, no
  render delta).

### M4 — Interrupt mode

- Wire the mode bits: SetMouse enabling VBL / movement / button
  interrupts makes the card assert the **M1 line** on the matching
  condition — VBL at the once-per-frame tick, movement when the fed
  position changed, button on a transition. ServeMouse reports the source
  and de-asserts; ReadMouse clears the "moved" flag.
- **Gate:** a headless end-to-end test — enable VBL interrupts, install a
  handler, run N frames, assert the handler fired once per frame and
  ServeMouse reported VBL, and that injected movement updates the
  position the handler reads. **Flagship:** boot a mouse-aware disk
  (MousePaint or a small mouse demo added to the test set) and a
  deterministic golden-BMP after a scripted pointer interaction; if no
  redistributable image is committable, the scripted firmware-level
  end-to-end test is the deterministic fallback gate (record the choice
  in `notes/MOUSE.md`).

### M5 — Docs + as-built note

- `notes/MOUSE.md`: the firmware protocol, the screen-hole/soft-switch
  map, the interrupt model and the **reusable IRQ line** (so Mockingboard
  inherits it), the coordinate decision — an as-built in the
  `APPLE_IIE_ENHANCED.md` style.
- README: a `two` mouse example (slot-4 mouse config), checked by
  `readme_examples_parse`; a committed `examples/` config it references.
- `notes/JSON_CONFIG.md` schema inventory gains `machine.slots.*.card =
  "mouse"`; `notes/IDEAS.md` ticks the AppleMouse item; `notes/STATE.md`
  notes the `Mou` `Persist` fields.
- **Gate:** `readme_examples_parse`; the standard gates.

## Hazards

- **The IRQ line is genuinely new machinery.** `cpu.irq()`'s current
  `push PC+1` is a BRK-shaped hack; a real IRQ must push the exact
  resume PC with B clear, or `RTI` returns to the wrong place and every
  interrupt-driven program corrupts. M1's gate pins the pushed PC/status
  precisely, and keeps BRK unchanged.
- **Per-instruction interrupt polling at ~1 MHz.** Do **not** scan
  devices each instruction — cache the line as one machine `bool` and
  refresh on state change; the burst check is `bool && I==0`. Called out
  so a naive first cut doesn't regress `two_timing.rs`.
- **Synthetic vs real firmware.** Software that bypasses the entry points
  and drives the 6821 PIA directly, or that depends on real `$C800`
  firmware code, will not work against synthetic firmware. This is rare
  (the entry points are the sanctioned interface), but record it: the
  fallback is a real ROM + per-slot `$C800` expansion (backlog).
- **VBL fidelity.** RDVBL is faked (quirk #3); the mouse VBL interrupt is
  a *separate*, real once-per-frame tick. Software that cross-checks
  RDVBL against mouse-VBL timing could notice — acceptable, documented.
- **Determinism / golden-BMP.** Mouse input is host-driven and
  non-deterministic; keep it out of the golden-BMP configs (no mouse card
  there) and gate the mouse with scripted-injection tests so the
  screenshot tripwire stays meaningful.
- **Coordinate mapping edge cases** — clamp changes mid-session, HomeMouse
  vs the host cursor, the window aspect vs the clamp window: pin these in
  M3's test, not by eyeballing MousePaint.
- **Autostart trap** — `$Cn01` must not be `$20` (the Disk II boot
  signature), the same footgun `clk.rs` documents; the golden ROM test
  asserts it.

## Decisions to make at kickoff

1. **Coordinate model** — relative/captured (recommended, faithful) or
   absolute/mapped (uncaptured, friendlier). Drives M3 and whether
   `SlotCard::Mouse` carries a `coords` field.
2. **Clamp defaults & config** — ship the `0..=1023` firmware default and
   leave clamp entirely to software (recommended), or expose clamp/coords
   as `mouse` card fields.
3. **Flagship gate image** — commit a redistributable mouse demo /
   MousePaint image for a golden-BMP end-to-end, or gate M4 with the
   scripted firmware-level test only.
4. **IRQ line home** — a method on `Two` polled by the burst loops
   (smaller, machine-local) vs a `Device`-trait `irq()` aggregated by
   `Memory` (more general). Recommend the `Two`-local line now; generalize
   when Mockingboard lands.
5. **PR granularity** — one PR per phase (default) or the whole plan in
   one.

## Backlog (recorded, out of scope)

- **Real AppleMouse ROM + per-slot `$C800` expansion** — the faithful
  path for software that needs the genuine firmware; needs the expansion
  latch EWM currently omits.
- **Mockingboard on the shared IRQ line** — M1's line is built to be
  reused; the 6522/AY device is the next customer (`notes/IDEAS.md`).
- **`//c` built-in mouse** — the //c wires the same firmware to its IOU;
  once the //c model exists (IDEAS), this card's `Mou`/`mouse_rom` are the
  substrate.
- **Mouse-as-paddles / KoalaPad** — a separate `notes/IDEAS.md` input
  item; unrelated to the card but shares the SDL mouse-event plumbing M3
  adds.
