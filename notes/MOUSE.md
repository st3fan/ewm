# The AppleMouse II card — as built

The mouse card, `{"card": "mouse"}`, any slot 1–7 (slot 4 is the canonical
home), both ][+ and //e. As built it is the card's **real hardware** — a 6520
PIA + a 6805 microcontroller + the card's own `342-0270-C` ROM — so *any*
mouse software drives it, including the //e path MousePaint uses.

Plans: `plans/20260721-01-apple-mouse-card.md` built a first, *synthetic*
firmware version (M1–M5); `plans/20260721-03-mouse-pia-hardware.md` (P1–P5)
replaced it with the real hardware after the synthetic card could not drive
MousePaint on the //e. This note is the *as-built* record of the real card.

Ported from the MIT `oliverschmidt/mouse-interface` reference (`PIA6520.c` —
the PIA; `MouseInterfaceCard.c` — the 6805 controller and the documented
6502↔6805 handshake). The ROM is from `freitz85/AppleIIMouse`.

## The three pieces (`ewm/src/mouse.rs`)

- **6520 PIA** (`Pia`) at the slot DEVSEL (`$C0nX`, offsets 0-3 =
  `PRA`/`CRA`/`PRB`/`CRB`; the low two address bits decode the four
  registers). `CRx bit 2` selects the data port vs. its direction register.
  Physical port value = `(OR & DDR) | (IN & ~DDR)`. Port A is the data byte
  to/from the 6805; port B is the handshake + ROM-bank select + a sync bit.
  The PIA's own IRQ is unused on this card and is not modelled.
- **6805 controller** (`Ctl`) — the mouse state (current/last position and
  buttons, the clamp window, the operating mode, the interrupt state) and the
  command engine. The 6502 sends 1-5 byte commands over port A gated by the
  port-B handshake; some commands reply.
- **Banked ROM** — the 2 KB `342-0270-C` ROM as eight 256-byte pages at
  `$Cn00`; PIA **port B bits 1-3** select which page is visible. The
  page-switch code sits at `$xx70` of every page, so the firmware flips banks
  mid-routine and execution continues on the new page. **No `$C800`
  expansion.** Provenance: sha1 `3a9d881a8a8d30f55b9719aceebbcf717f829d6f`,
  pinned by `mouse_rom_is_the_committed_image`, committed as
  `roms/Apple Mouse Interface Card ROM - 342-0270-C.bin`.

### Identification

Read by the Autostart scan and mouse-aware software, from the default bank
(page 0) of the real ROM:

| offset | value | meaning |
|---|---|---|
| `$Cn01` | `≠ $20` | **not** the Disk II boot signature |
| `$Cn05` | `$38` | Pascal 1.1 firmware protocol |
| `$Cn07` | `$18` | Pascal 1.1 firmware protocol |
| `$Cn0C` | `$20` | X-Y pointing device |
| `$CnFB` | `$D6` | AppleMouse |

## Port B bits and the handshake

Port B (DDR = `0x3E`, bits 1-5 output): bit0 sync latch; bits1-3 ROM page
(A8-A10); bit4 RDACK; bit5 WRREQUEST; bit6 RDREADY; bit7 WRACK. The 6805
drives the slot IRQ line directly.

**Write (6502→6805):** the 6502 sets WRREQUEST and waits WRACK; the 6805
latches port A and sets WRACK; the 6502 clears WRREQUEST and waits ¬WRACK; the
6805 clears WRACK. **Read (6805→6502):** the 6805 puts a byte on port A and
raises RDREADY; the 6502 reads it, sets RDACK, waits ¬RDREADY; the 6805 clears
RDREADY; the 6502 clears RDACK. **There are no timeouts** — a wrong step hangs
the firmware forever (this was the synthetic card's failure).

EWM has no second core, so the 6805's run loop is a state machine advanced to
a **fixpoint after every PIA write** (the write is the only thing that moves
port B), plus the per-frame VBL tick. The reference's anti-hang guard (offer
RDREADY whenever idle) is preserved.

## Commands

Top nibble of the first byte: SETMOUSE `$0n` (mode in bits 0-3), READMOUSE
`$1n` (→ Xhi,Xlo,Yhi,Ylo,status), SERVEMOUSE `$2n` (→ status; clears the IRQ),
CLEARMOUSE `$3n`, POSMOUSE `$4n` (+4), INITMOUSE `$5n` (clamp `0..=1023`, home,
IRQ off), CLAMPMOUSE `$6n` (+4; bit0 = X/Y), HOMEMOUSE `$7n`, TIMEMOUSE `$9n`
(50/60 Hz), RDMEMMOUSE `$Fn` (+2; the GETCLAMP workaround). The command bodies
are a near-verbatim port of `MouseInterfaceCard.c`.

**Mode bits** (SetMouse): 0 = mouse on; 1 = interrupt on movement; 2 =
interrupt on button; 3 = interrupt on VBL (active even without "on").
**Status bits:** 7 = button 0 down now; 6 = button 0 down at last read; 5 =
moved since last read; 3/2/1 = the VBL/button/movement interrupt source.

## Calling convention and screen holes (how software drives it)

The ROM's routines are found through the `$Cn12` offset table (SetMouse=0,
ServeMouse=1, ReadMouse=2, ClearMouse=3, PosMouse=4, ClampMouse=5,
HomeMouse=6, InitMouse=7) and called with the **Apple II slot-firmware
register convention**: `X = $Cn` (the ROM-page high byte, for indexing the
screen holes) and `Y = slot×16` (the `$C0nX` DEVSEL offset). The `$Cn00`
Pascal entries handle interrupts / auto-detect the slot; the direct routine
calls require X/Y set by the caller.

The routines move state through the caller's per-slot **screen holes**, with
one documented quirk:

- **PosMouse / ReadMouse** use the slot-`n` holes: X = (`$0478+n` lo,
  `$0578+n` hi), Y = (`$04F8+n` lo, `$05F8+n` hi). ReadMouse also deposits the
  status byte at `$0778+n`; the mode is at `$07F8+n`.
- **ClampMouse** is the exception — it reads its bounds from the **fixed
  slot-0 holes regardless of slot**: min = (`$0478` lo, `$0578` hi), max =
  (`$04F8` lo, `$05F8` hi). This is the documented mouse-clamping quirk (Apple
  II Technical Note Mouse #7), the reason the RDMEMMOUSE/GETCLAMP workaround
  exists.

## Interrupts and the reusable IRQ line

The 6805 drives the slot IRQ, wired to `Two`'s single cached `irq_line` (the
maskable-interrupt path M1, reused by Mockingboard next):

- **`cpu.irq()` is a real hardware IRQ**: it pushes the exact resume PC with
  **B clear** and vectors `$FFFE`, distinct from BRK. The ][+ Autostart ROM
  routes such an interrupt through the user vector `$03FE`.
- **`service_irq`**, called between `cpu.step()`s in both burst loops, takes
  the IRQ when `irq_line` is high and `I == 0`, then re-derives the line from
  the device so a handler's ServeMouse de-assert mid-burst is never re-taken.
  Never scanned per instruction.
- **`tick_vbl()`** runs once per frame: it pulses the mouse's VBL (raising the
  VBL interrupt if enabled) and refreshes the line. Movement / button
  interrupts refresh on the host feed. `Two::mouse_irq_pending()` exposes the
  asserted state for tests.
- **ServeMouse** reports the source in the status hole and clears it.

On the **][+** this is proven end-to-end through the real firmware
(`vbl_interrupts_fire_once_per_frame_and_serve_reports_the_source`). The
**//e** interrupt path is different and not yet complete — see *Not done*.

## Host input — absolute/mapped

Both the SDL window pointer and the RFB/VNC pointer map their pixel
proportionally into the mouse's clamp window through `Two::feed_mouse_pixel`
(→ `Mou::set_position` + `set_button`); `Mou::move_by` (relative integration)
remains for a future captured mode. The device model is the reference's
`mouseControllerMoveXY`.

## EWM integration

One `Mou` device is mapped to both the DEVSEL (the PIA) and `$Cn00` (the
banked ROM) via `add_device` + `map_device`; `read` decodes by page (`$C0` →
PIA, else → ROM). On the //e the `$Cn00` region shadows the IOU's `$CX`-ROM
mapping (regions are walked newest-first), so the banked ROM is served under
INTCXROM arbitration. Full state round-trips via `Persist`.

## What MousePaint proves

MousePaint on the //e — which hung with the synthetic card because it drives
the card as its real hardware, not the `$Cn00` entry points — now boots to its
menu and responds to the keyboard (the mouse-gated menu advances on RETURN).
`tests/two_mouse_iie.rs` is the committable stand-in: a fed host pointer is
reported by the real ROM's ReadMouse on a //e.

## Not done (backlog, per the plan)

- **//e interrupt-driven mouse via the //e ROM handler**: the //e's `$FFFE`
  handler (`$C3FA`) enables INTCXROM and enters the card firmware's own
  interrupt entry (`$C400`, V-set) rather than `$03FE`; that firmware routine
  loops against our card (it expects a different interrupt-service reply than
  ServeMouse). The ][+ interrupt path and the //e polling path both work.
- Mockingboard on the shared IRQ line; the `//c` built-in mouse (this card is
  the substrate); mouse-as-paddles / KoalaPad.
