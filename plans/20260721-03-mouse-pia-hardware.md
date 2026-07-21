# Real AppleMouse hardware — 6520 PIA + 6805 controller + the 342-0270-C ROM

- **Design docs:** self-contained (below), building on the synthetic card's
  as-built `notes/MOUSE.md` (and its "Known limitation" — the diagnosis this
  answers). Background: `notes/JSON_CONFIG.md` (slots), `notes/STATE.md`
  (`Persist`), `plans/20260721-01-apple-mouse-card.md` (the synthetic card
  this replaces). **Reference implementations** (the roadmap — we port these):
  - `github.com/oliverschmidt/mouse-interface` (MIT) — `PIA6520.c` (the 6520
    PIA) and `MouseInterfaceCard.c` (the 6805 controller + the 6502↔6805
    handshake, fully documented in its header comment).
  - `github.com/freitz85/AppleIIMouse` — the real card ROM (**committed** as
    `roms/Apple Mouse Interface Card ROM - 342-0270-C.bin`, 2 KB, sha1
    `3a9d881a8a8d30f55b9719aceebbcf717f829d6f`) and its disassembly.
- **Status:** complete — P1–P5 landed (one PR per phase). The card is the real
  6520 PIA + 6805 + `342-0270-C` ROM; MousePaint on the //e works. One item in
  the backlog (the //e interrupt-driven path via the //e ROM handler).
- **Target:** `main`; one PR per phase (default).

## Why (the diagnosis, now with the reference)

MousePaint hangs on the //e: it drives the card as its **real hardware**, not
the synthetic firmware's entry points (`notes/MOUSE.md` "Known limitation").
The reference nails what that hardware is:

- A **6520/6521 PIA** at the slot DEVSEL (`$C0nX`, offsets 0-3: `PRA`/`CRA`/
  `PRB`/`CRB`, `CRx bit 2` selects data vs DDR).
- A **6805 microcontroller** — the mouse brain. The 6502 firmware talks to it
  through the PIA: **port A = data**, **port B = handshake + ROM bank +
  sync**. *The firmware has no timeouts — a wrong handshake hangs it, which is
  exactly today's symptom.*
- A **2 KB ROM** banked into the 256-byte `$Cn00` slot area as **8 pages,
  selected by PIA port B bits 1-3** (the page-switch code sits at `$xx70` so
  it can switch mid-execution). **No `$C800` expansion** — a real
  simplification over the prior revision's guess.

So the card works on **both ][+ and //e** with its own ROM; there is no
//e-vs-][+ split, and no `$C800` infra to build.

## The design (what we port)

The mouse card device = a **6520 PIA** + a **6805 model** + the **banked
342-0270-C ROM**, all in the slot.

**PIA port B bits** (`MouseInterfaceCard.c`): bit0 = sync latch; bits1-3 =
ROM page select (A8-A10); bit4 = RDACK; bit5 = WRREQUEST; bit6 = RDREADY;
bit7 = WRACK. Port B DDR is `0x3E` (bits1-5 out). Port A = the data byte.

**Handshake** (no timeouts): write (6502→6805) — 6502 sets WRREQUEST, waits
WRACK, 6805 latches port A + sets WRACK, 6502 clears WRREQUEST, waits ¬WRACK,
6805 clears WRACK. Read (6805→6502) — 6805 sets RDREADY, 6502 reads port A +
sets RDACK, waits ¬RDREADY, 6805 clears RDREADY, 6502 clears RDACK.

**Commands** (top nibble; params written first, then the command; some reply):
SETMOUSE `$0n` (mode in bits 0-3), READMOUSE `$1n` (→5 bytes: Xhi,Xlo,Yhi,Ylo,
status), SERVEMOUSE `$2n` (→1 byte, clears the IRQ), CLEARMOUSE `$3n`,
POSMOUSE `$4n` (+4), INITMOUSE `$5n` (clamp 0..=1023), CLAMPMOUSE `$6n` (+4,
bit0 = X/Y), HOMEMOUSE `$7n`, TIMEMOUSE `$9n` (50/60 Hz), RDMEMMOUSE `$Fn`
(+2, the GETCLAMP workaround). Mode bits: 0 on, 1 move-IRQ, 2 button-IRQ, 3
VBL-IRQ (VBL fires even without "on"). Status bits per the header. The 6805
state (Current/Last X/Y/buttons, Clamp, mode, IntState) and every command's
body are given verbatim in `MouseInterfaceCard.c` — a near-direct Rust port.

**Interrupts:** the 6805 drives the **slot IRQ line directly** (the PIA's own
IRQ is unused) — assert on VBL / movement / button per the mode; SERVEMOUSE
clears. Wire to the `Two`-local IRQ line M1 built. VBL = 17030 cycles (60 Hz),
matching `tick_vbl`.

**In EWM's synchronous model:** the 6805 has no real concurrency — model it as
a state machine advanced by the PIA accesses (the handshake) and the per-frame
VBL tick. This **replaces** the synthetic `mouse_rom`/`Mou` (P5); the config
surface (`{"card":"mouse"}`) is unchanged.

## Phases

| Phase | Description | Size | Status |
|---|---|---|---|
| P1 | The card substrate: 6520 PIA + 6805 controller + handshake + all commands + banked 342-0270-C ROM at `$Cn00`; retire the synthetic `mouse_rom`; migrate the unit tests | L | **Landed** (#332) |
| P2 | The **real ROM firmware** drives it end-to-end (the entry convention + `$xx70` page-switching): Init→Clamp→Pos→Read in the screen holes | M | **Landed** (#TBD) |
| P3 | Host input → the 6805; the MousePaint flagship (boots to a working pointer on the //e) | M | **Landed** (#TBD) |
| P4 | Interrupts through the real firmware (][+): VBL asserts the line, ServeMouse reports + clears | M | **Landed** (#TBD) |
| P5 | Docs (`notes/MOUSE.md` as-built); final cleanup | S | **Landed** (#TBD) |

**As built (P1):** the PIA, the 6805 controller (handshake + every command
body), the banked ROM, host input, and the interrupt model are too tightly
coupled to split, so P1 landed them together — a self-contained `mouse.rs`
ported near-verbatim from the reference, unit-tested (PIA registers, port-B
banking, a simulated-6502 handshake driving Init→Clamp→Pos→Read, movement/
clamp, interrupt gating, state round-trip) plus ROM provenance and the
identification bytes read through the real machine bus. The synthetic
`mouse_rom`/`Mou` protocol and its M2–M4 unit tests are retired/migrated. What
remains for P2 is proving the **real 6502 ROM code** runs the flow end-to-end
(the two `tests/two_mouse.rs` firmware tests are `#[ignore]`d until then).

Order **P1 → P5**. Every phase: standard gates (`cargo fmt --all --check`,
`cargo clippy --all-targets -- -D warnings`, full `cargo test` **including the
golden-BMP screenshots** — no mouse in those configs) plus the phase gate.

### P1 — the PIA + the banked ROM

- Embed `342-0270-C.bin` (provenance test, SHA-1). A `Mou` device carrying a
  6520 PIA (reuse/extend `ewm/src/pia.rs`) at the DEVSEL; the `$Cn00` slot ROM
  is one of the 8 ROM pages, selected by PIA port B bits 1-3. This needs the
  slot-ROM region to be **bank-switchable** (today it is a fixed 256 bytes) —
  the card serves `$Cn00` itself and returns the selected page.
- **Gate:** the ROM provenance test; the ID bytes (`$Cn05=$38`, `$Cn07=$18`,
  `$CnFB=$D6`) read from the default bank; a test that writing port B bits 1-3
  changes which ROM page is visible at `$Cn00`.

### P2 — the real ROM firmware drives it end-to-end

- The controller/handshake landed in P1; P2 proves the **real 6502 ROM** runs
  the flow. `tests/two_mouse.rs`'s
  `init_clamp_pos_read_through_the_firmware_deposits_clamped_holes` runs the
  ROM's entry points (found via the `$Cn12` table) and asserts the clamped
  X/Y/status land in the screen holes.
- **Gate (met):** the real firmware does Init→Clamp→Pos→Read; the position
  clamps to (700, 200) and reads back through the screen holes.

**As built (P2):** the real ROM needed two conventions the retired synthetic
firmware didn't:
1. **Register calling convention** — the ROM's routines require `X = $Cn`
   (the ROM-page high byte, for indexing the screen holes) and `Y = slot×16`
   (the `$C0nX` DEVSEL offset), set by the caller before each JSR. The card's
   banking + PIA handshake then run correctly, including the ROM's `$xx70`
   mid-routine page-switching (verified: `$C400`/`$C470` execute different
   per-bank code as port B flips banks 2→6→…).
2. **Screen-hole layout** — Pos/Read use the slot-4 holes (X = `$047C`/`$057C`,
   Y = `$04FC`/`$05FC`); **ClampMouse reads the *fixed* slot-0 holes**
   (`$0478`/`$0578` min, `$04F8`/`$05F8` max) regardless of slot — the
   documented clamp quirk (Apple II Technical Note Mouse #7, the reason the
   RDMEMMOUSE/GETCLAMP workaround exists). No card change was needed; the P1
   controller was already correct.

### P3 — host input + the flagship

- Host input (`feed_mouse_*` → `set_position`/`move_by`/`set_button`) landed
  in P1; P3 proves it reaches a program through the **real firmware on a //e**
  (MousePaint's environment).
- **Gate (met):** `tests/two_mouse_iie.rs` — on an Enhanced //e, a fed host
  pointer (position + button) is reported by the real ROM's ReadMouse in the
  screen holes; the banked `$Cn00` ROM is served through the //e IOU's
  `$CX`-ROM region (INTCXROM shadowing), and the card identifies from it.

**As built (P3):** no host-input code change was needed (P1 already wired it);
P3 added the //e firmware-level flagship. **Dev-time confirmation:** the actual
MousePaint disk (680-0239-A, not committed) now boots to its menu and
**responds to RETURN** — the screen advances from "Press RETURN to learn how to
use the mouse" to "One moment, please." The reported hang (the mouse-gated menu
ignoring the keyboard) is resolved.

### P4 — interrupts

- The 6805 asserts the M1 IRQ line on VBL (per-frame, if VBL-IRQ mode) /
  movement / button; SERVEMOUSE reports the source and de-asserts.
- **Gate (met, ][+):** `tests/two_mouse.rs`'s
  `vbl_interrupts_fire_once_per_frame_and_serve_reports_the_source` — VBL
  interrupts enabled through the real SetMouse, a handler installed at the ROM
  user IRQ vector `$03FE` calls the real ServeMouse, and it fires exactly once
  per frame with ServeMouse reporting the VBL source (`$08`) in the status
  hole and de-asserting the line (`Two::mouse_irq_pending()` confirms no
  re-fire).

**As built (P4):** on the **][+** the interrupt path works end-to-end through
the real firmware. On the **//e** the IRQ vectors differently — `$FFFE`
(`$C3FA`) enables INTCXROM and jumps into the **card firmware's own interrupt
entry** (`$C400`, V-set), not the `$03FE` user vector — and that firmware
interrupt-service routine does not complete against our card (it reads a
multi-byte handshake stream looking for a terminator our ServeMouse response
does not match, and loops). MousePaint's //e path is **polling**, which works
(P3), so this does not block the flagship. Recorded in the backlog.

### P5 — retire the synthetic card + docs

- Delete `mouse_rom` and the synthetic `Mou` protocol; the config/machine
  wiring points at the real card. Migrate the M2–M4 tests (their subject
  changed). `notes/MOUSE.md` rewritten as the real-hardware as-built.
- **Gate:** the migrated tests; standard gates.

## Hazards

- **The handshake must be exact.** The firmware has no timeouts — any
  deviation hangs it (today's symptom). Port the reference's handshake state
  machine faithfully and test each transfer.
- **`$Cn00` ROM banking is new.** EWM's slot ROM is a fixed 256 bytes; the
  card must serve a port-B-selected page. The `$xx70` page-switch trick means
  the *code itself* switches banks mid-run — the region must reflect port B
  live.
- **No real concurrency.** The reference's 6805 runs on its own core with a
  VBL sync handler; here it is a state machine driven by PIA accesses + the
  per-frame VBL tick. Deterministic, which golden-BMP requires.
- **//e IRQ path** differs from the ][+ `$03FE` path the last plan tested (the
  //e ROM handler saves MMU state first) — P4 must exercise the real handler.
- **Replacing the synthetic card** churns `plans/20260721-01`'s M2–M4 tests;
  migrate, don't leave two DEVSEL meanings.
- **The MousePaint disk is not redistributable** — the flagship uses a
  dev-time download; the CI gate is a firmware-level assertion.

## Decisions to make at kickoff

1. **Flagship gate** — committable firmware-level assertion (recommended,
   CI-safe) vs. golden-BMP needing the non-redistributable disk (dev-only).
2. **PR granularity** — one per phase (default) or fewer.

## Backlog (recorded, out of scope)

- **//e interrupt-driven mouse via the //e ROM handler** (P4 as-built): the
  //e's `$FFFE` handler (`$C3FA`) enables INTCXROM and enters the card
  firmware's own interrupt entry (`$C400`, V-set) rather than the `$03FE` user
  vector; that firmware routine loops against our card instead of completing.
  The ][+ interrupt path and the //e polling path both work. Needs the card's
  interrupt-service handshake response to match what the //e firmware entry
  expects (likely a different reply format/terminator than ServeMouse).
- Mockingboard on the shared IRQ line; the `//c` built-in mouse (the //c wires
  a similar controller to its IOU); the unknown commands `$8n`/`$An`/`$Bn`/
  `$Cn` (the 6805 leaves them unimplemented too).
