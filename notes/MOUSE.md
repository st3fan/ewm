# The AppleMouse II card — as built

The mouse card, `{"card": "mouse"}`, any slot 1–7 (slot 4 is the canonical
home), both ][+ and //e. Plan: `plans/20260721-01-apple-mouse-card.md`
(M1–M5, all landed). This note is the *as-built* record — the firmware
protocol, the private device wire, the interrupt model, and the decisions
that diverged from the plan.

## Synthetic firmware, not the real ROM

Like the Thunderclock (`clk.rs`) and hard drive (`hdd.rs`), the card is
**hand-assembled firmware that implements the documented protocol**, backed
by a Rust `Device` — not a 6821 PIA + 68705 simulation. The real AppleMouse
firmware spills into `$C800-$CFFF` expansion ROM, which EWM does not model;
synthetic firmware living entirely in the 256-byte `$Cn00` page sidesteps
that, and the eight documented entry points are the interface every mouse
program (MousePaint, GEOS, Dazzle Draw, the ProDOS `MOUSE.OBJ` driver) calls.

`mouse_rom(slot)` (in `ewm/src/mouse.rs`) is a per-slot generator, mirroring
`clk_rom(slot)`: it assembles the eight routines sequentially from `$Cn1A`,
records their low bytes in the offset table at `$Cn12`, and patches the
slot-dependent operands (the DEVSEL port low byte, the screen-hole low
bytes). Pinned byte-for-byte by `mouse_rom_slot4_is_golden`.

### Identification

Read by the Autostart scan and mouse-aware software:

| offset | value | meaning |
|---|---|---|
| `$Cn01` | `≠ $20` | **not** the Disk II boot signature (the `clk.rs` trap) |
| `$Cn05` | `$38` | Pascal 1.1 firmware protocol |
| `$Cn07` | `$18` | Pascal 1.1 firmware protocol |
| `$Cn0B` | `$01` | Pascal 1.1 firmware protocol |
| `$Cn0C` | `$20` | X-Y pointing device |
| `$CnFB` | `$D6` | AppleMouse |

### The eight routines and the screen holes

Found through the `$Cn12-$Cn19` offset table (low bytes, fixed order:
SetMouse, ServeMouse, ReadMouse, ClearMouse, PosMouse, ClampMouse, HomeMouse,
InitMouse). They move state between the caller's per-slot **screen holes** and
the card's DEVSEL ports:

| hole | slot 4 | content (after ReadMouse) |
|---|---|---|
| `$0478+n` / `$04F8+n` | `$047C` / `$04FC` | X low / high |
| `$0578+n` / `$05F8+n` | `$057C` / `$05FC` | Y low / high |
| `$0778+n` | `$077C` | status byte |
| `$07F8+n` | `$07FC` | mode byte |

- **ReadMouse** latches the device and copies X/Y/status/mode into the holes.
- **SetMouse** (A = mode) sets the mode byte (below).
- **PosMouse** forces a position from the X/Y holes; **ClampMouse** (A = 0 for
  X, 1 for Y) takes the clamp minimum from the X holes and maximum from the Y
  holes; **HomeMouse** moves to the clamp minimum; **ClearMouse** zeroes the
  position; **InitMouse** resets to defaults (clamp `0..=1023`, mouse off).
- **ServeMouse** reads and clears the pending interrupt source (the interrupt
  model, below).

The **status byte**: bit 7 = button down now, bit 6 = button down at the last
read, bit 5 = moved since the last read.

The **mode byte** (SetMouse): bit 0 = mouse on; bit 1 = interrupt on movement;
bit 2 = interrupt on button; bit 3 = interrupt on VBL.

## The DEVSEL protocol — our private wire (`Mou`)

Only our firmware touches the slot's 16-byte DEVSEL range (`$C080 + slot*16`,
low nibble decoded), so the assignment is ours. `Mou` holds the 16-bit
position, the per-axis clamp, the button (now + at last read), the moved flag,
the mode, the latched read stream, four parameter bytes, and the pending
interrupt source.

| offset | read | write |
|---|---|---|
| 0 | status byte | **latch**: snapshot X/Y/button, rewind the read stream |
| 1 | next latched byte: Xlo, Xhi, Ylo, Yhi, status, mode (auto-increment) | — |
| 2 | mode | set mode |
| 4-7 | — | parameter bytes (Xlo, Xhi, Ylo, Yhi — reused as clamp min/max) |
| 8 | — | command: 0 SetPos, 1 Init, 2 Clear, 3 Home, 4 ClampX, 5 ClampY |
| 9 | interrupt source, cleared on read (ServeMouse) | — |

ReadMouse is the mirror of `clk_rom`'s streamed read: one write to latch, then
six reads off port 1. Multi-byte parameters (position, clamp bounds) stream in
through ports 4-7, and port 8 dispatches the command that consumes them.

## The interrupt model and the reusable IRQ line

The mouse is the first user of EWM's maskable-interrupt path (M1), built to be
reused (Mockingboard is the next customer — `notes/IDEAS.md`).

- **`cpu.irq()` is a real hardware IRQ**: it pushes the *exact* resume PC with
  **B clear** and vectors `$FFFE`, distinct from BRK (`brk_interrupt`: PC+1,
  B set). This is what makes the ROM's IRQ handler route a hardware interrupt
  through the user vector `$03FE` instead of mistaking it for a BRK — the M4
  end-to-end test installs its handler exactly that way.
- **The line is a single cached `bool` on `Two`** (`irq_line`), the OR of the
  interrupt-capable devices' asserted state. `service_irq`, called between
  `cpu.step()`s in both burst loops, takes the IRQ when the line is high and
  `I == 0`. The common case is a cheap `bool && I` check; only a cached-high
  line is re-derived from the device, so a handler's ServeMouse de-assert
  mid-burst is never re-taken. **Never scanned per instruction** — the plan's
  headline hazard.
- **`tick_vbl()` runs once per frame** (60 Hz, deterministic, matching the
  frame loop the golden-BMP culture requires): it pulses the mouse's VBL and
  refreshes the line. Movement and button interrupts refresh on the host feed.
- **ServeMouse** (read of port 9) reports the source (VBL / movement / button
  bits) and clears it, de-asserting the line.

## Host input — the coordinate decision (revised)

The plan recommended **relative/captured** (SDL relative-mouse mode, integrate
deltas). **As built it is absolute/mapped** (decided during M3): both the SDL
window pointer and the RFB/VNC pointer map their pixel *proportionally into the
mouse's clamp window* through one `Two::feed_mouse_pixel`. Why the change: no
cursor grab/release machinery, it is testable headlessly, and it is friendlier
for a windowed emulator (the plan's documented alternative). `Mou::move_by`
(relative integration) remains, so a captured relative mode is a small future
addition. The SDL frontend feeds the device on `MouseMotion` / `MouseButton`
events; the RFB serve path routes a `PointerEvent` to the mouse when a card is
present (`mask` bit 0 = button), else the pre-existing paddle-0 fallback.

## Not done (backlog, per the plan)

Real AppleMouse ROM + per-slot `$C800` expansion (for software that bypasses
the entry points); Mockingboard on the shared IRQ line; the `//c` built-in
mouse (this `Mou`/`mouse_rom` is the substrate); mouse-as-paddles / KoalaPad.
