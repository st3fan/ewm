//! The AppleMouse II card (`{"card": "mouse"}`, slot 4 by default, any slot
//! 1–7). Like the Thunderclock and hard drive, this is *synthetic firmware*
//! — a 256-byte `$Cn00` ROM implementing the documented eight-entry mouse
//! protocol, backed by a Rust `Device` in the slot's DEVSEL range — not a
//! simulation of the card's 6821 PIA + 68705 microcontroller
//! (plans/20260721-01). The firmware fits entirely in `$Cn00`, sidestepping
//! the `$C800` expansion EWM does not model.
//!
//! ## Firmware protocol (what software sees)
//!
//! Identification, per the Apple II Pascal 1.1 firmware protocol + the mouse
//! ID: `$Cn05=$38`, `$Cn07=$18`, `$Cn0B=$01`, `$Cn0C=$20` (X-Y pointing
//! device), `$CnFB=$D6` (AppleMouse). `$Cn01 ≠ $20`, so the Autostart slot
//! scan never mistakes the card for a bootable Disk II (the trap `clk.rs`
//! documents).
//!
//! The eight routines are found through the offset table at `$Cn12-$Cn19`
//! (one low byte each, in the fixed order SetMouse, ServeMouse, ReadMouse,
//! ClearMouse, PosMouse, ClampMouse, HomeMouse, InitMouse), and move state
//! between the caller's per-slot **screen holes** and the card's DEVSEL soft
//! switches:
//!
//! - `ReadMouse` latches the device, then deposits X (lo/hi), Y (lo/hi), the
//!   status byte and the mode into the screen holes `$0478+n` / `$04F8+n`
//!   (X), `$0578+n` / `$05F8+n` (Y), `$0778+n` (status), `$07F8+n` (mode).
//! - `SetMouse` takes the mode byte in A (bit0 = mouse on; bits 1-3 =
//!   interrupt on movement / button / VBL).
//! - `PosMouse` forces a position from the X/Y screen holes; `ClampMouse`
//!   (A = 0 for X, 1 for Y) takes the clamp min in the X holes and max in the
//!   Y holes; `HomeMouse` moves to the clamp minimum; `ClearMouse` zeroes the
//!   position; `InitMouse` resets to defaults (clamp `0..=1023`, mouse off).
//!   `ServeMouse` reports and clears the interrupt source (Phase M4).
//!
//! ## The DEVSEL protocol (our private wire, firmware ↔ `Mou`)
//!
//! Only our firmware touches these ports, so the assignment within the
//! slot's 16-byte DEVSEL range (`$C080 + slot*16`, low nibble decoded) is
//! ours to choose:
//!
//! | offset | read | write |
//! |---|---|---|
//! | 0 | status byte | **latch**: snapshot X/Y/button, rewind the read stream |
//! | 1 | next latched byte: Xlo, Xhi, Ylo, Yhi, status, mode (auto-increment) | — |
//! | 2 | mode | set mode |
//! | 4-7 | — | parameter bytes (Xlo, Xhi, Ylo, Yhi — reused as clamp min/max) |
//! | 8 | — | **command**: 0 SetPos, 1 Init, 2 Clear, 3 Home, 4 ClampX, 5 ClampY |
//! | 9 | interrupt source (ServeMouse), cleared on read | — |

use ewm_core::mem::Device;

/// The default clamp window: `0..=1023` on both axes (the firmware default a
/// program overrides with ClampMouse).
const CLAMP_MIN: i32 = 0;
const CLAMP_MAX: i32 = 1023;

/// Mode byte bits (SetMouse): mouse enabled, and interrupt-on-movement /
/// button / VBL. The interrupt-source byte ServeMouse reports reuses the
/// movement / button / VBL bits.
const MODE_ON: u8 = 0x01;
const INT_MOVE: u8 = 0x02;
const INT_BUTTON: u8 = 0x04;
const INT_VBL: u8 = 0x08;

/// Build the per-slot 256-byte firmware. The routines are assembled
/// sequentially from `$Cn1A`; the offset table at `$Cn12` records where each
/// landed. Slot-dependent operands — the DEVSEL port low byte and the
/// screen-hole low bytes — are patched in, exactly as `clk_rom` patches its
/// two ports. Pinned byte-for-byte by `mouse_rom_slot4_is_golden`.
pub fn mouse_rom(slot: u8) -> [u8; 256] {
    let base = 0x80 + slot * 16; // DEVSEL port low byte; the page is $C0
    let lo = 0x78 + slot; // screen-hole low byte in pages $04/$05/$07 (X/Y lo, status)
    let hi = 0xf8 + slot; // screen-hole low byte (X/Y hi, mode)

    // Each routine, in offset-table order. `8D ll hh` = STA $hhll,
    // `AD ll hh` = LDA $hhll, `A9 ii` = LDA #ii.
    let set_mouse = vec![
        0x8d,
        base + 2,
        0xc0, // STA mode port
        0x8d,
        hi,
        0x07, // STA $07F8+n (mode screen hole)
        0x60, // RTS
    ];
    let serve_mouse = vec![
        0xad,
        base + 9,
        0xc0, // LDA serve port (clears the source)
        0x60, // RTS
    ];
    let read_mouse = vec![
        0x8d,
        base,
        0xc0, // STA latch port (A ignored)
        0xad,
        base + 1,
        0xc0,
        0x8d,
        lo,
        0x04, // LDA stream; STA $0478+n (Xlo)
        0xad,
        base + 1,
        0xc0,
        0x8d,
        hi,
        0x04, // Xhi -> $04F8+n
        0xad,
        base + 1,
        0xc0,
        0x8d,
        lo,
        0x05, // Ylo -> $0578+n
        0xad,
        base + 1,
        0xc0,
        0x8d,
        hi,
        0x05, // Yhi -> $05F8+n
        0xad,
        base + 1,
        0xc0,
        0x8d,
        lo,
        0x07, // status -> $0778+n
        0xad,
        base + 1,
        0xc0,
        0x8d,
        hi,
        0x07, // mode -> $07F8+n
        0x60, // RTS
    ];
    let clear_mouse = vec![0xa9, 0x02, 0x8d, base + 8, 0xc0, 0x60]; // cmd 2
    let pos_mouse = vec![
        0xad,
        lo,
        0x04,
        0x8d,
        base + 4,
        0xc0, // LDA $0478+n (Xlo); STA temp0
        0xad,
        hi,
        0x04,
        0x8d,
        base + 5,
        0xc0, // Xhi -> temp1
        0xad,
        lo,
        0x05,
        0x8d,
        base + 6,
        0xc0, // Ylo -> temp2
        0xad,
        hi,
        0x05,
        0x8d,
        base + 7,
        0xc0, // Yhi -> temp3
        0xa9,
        0x00,
        0x8d,
        base + 8,
        0xc0, // cmd 0 (SetPos)
        0x60,
    ];
    let clamp_mouse = vec![
        0x48, // PHA (save the axis)
        0xad,
        lo,
        0x04,
        0x8d,
        base + 4,
        0xc0, // min lo ($0478+n) -> temp0
        0xad,
        hi,
        0x04,
        0x8d,
        base + 5,
        0xc0, // min hi ($04F8+n) -> temp1
        0xad,
        lo,
        0x05,
        0x8d,
        base + 6,
        0xc0, // max lo ($0578+n) -> temp2
        0xad,
        hi,
        0x05,
        0x8d,
        base + 7,
        0xc0, // max hi ($05F8+n) -> temp3
        0x68, // PLA (axis)
        0x18,
        0x69,
        0x04, // CLC; ADC #4  -> cmd 4 (ClampX) or 5 (ClampY)
        0x8d,
        base + 8,
        0xc0, // STA command port
        0x60,
    ];
    let home_mouse = vec![0xa9, 0x03, 0x8d, base + 8, 0xc0, 0x60]; // cmd 3
    let init_mouse = vec![0xa9, 0x01, 0x8d, base + 8, 0xc0, 0x60]; // cmd 1

    let routines = [
        &set_mouse,
        &serve_mouse,
        &read_mouse,
        &clear_mouse,
        &pos_mouse,
        &clamp_mouse,
        &home_mouse,
        &init_mouse,
    ];

    let mut rom = [0u8; 256];
    // Identification bytes.
    rom[0x01] = 0x38; // != $20: not the Disk II boot signature
    rom[0x05] = 0x38; // Pascal 1.1 firmware protocol
    rom[0x07] = 0x18;
    rom[0x0b] = 0x01;
    rom[0x0c] = 0x20; // X-Y pointing device
    rom[0xfb] = 0xd6; // AppleMouse ID

    // Lay the routines out from $1A and record their offsets in the table.
    let mut cur = 0x1a;
    let mut table = [0u8; 8];
    for (i, r) in routines.iter().enumerate() {
        table[i] = cur as u8;
        rom[cur..cur + r.len()].copy_from_slice(r);
        cur += r.len();
    }
    rom[0x12..0x1a].copy_from_slice(&table);
    rom
}

/// The AppleMouse device over its slot's 16-byte DEVSEL range; only the low
/// nibble is decoded, so the same device works in any slot. Holds the 16-bit
/// position, the per-axis clamp window, the button state (now + at last
/// read), the "moved since last read" flag, the mode byte, and the four
/// parameter bytes the firmware streams in.
pub struct Mou {
    x: i32,
    y: i32,
    clamp_x: (i32, i32),
    clamp_y: (i32, i32),
    /// The host button, now and at the last `latch` (for click detection).
    button: bool,
    button_last: bool,
    /// Set when the position changed since the last `latch`.
    moved: bool,
    mode: u8,
    /// Latched snapshot the read stream serves (Xlo…mode), and the cursor.
    latched_x: u16,
    latched_y: u16,
    latched_status: u8,
    latched_mode: u8,
    read_index: usize,
    /// Parameter bytes written to ports 4-7 before a command.
    temp: [u8; 4],
    /// The interrupt source ServeMouse reports (Phase M4).
    irq_source: u8,
}

impl Mou {
    pub fn new() -> Mou {
        Mou {
            x: 0,
            y: 0,
            clamp_x: (CLAMP_MIN, CLAMP_MAX),
            clamp_y: (CLAMP_MIN, CLAMP_MAX),
            button: false,
            button_last: false,
            moved: false,
            mode: 0,
            latched_x: 0,
            latched_y: 0,
            latched_status: 0,
            latched_mode: 0,
            read_index: 6,
            temp: [0; 4],
            irq_source: 0,
        }
    }

    /// The live status byte: bit7 = button down now, bit6 = button down at the
    /// last read, bit5 = moved since the last read.
    fn status(&self) -> u8 {
        (self.button as u8) << 7 | (self.button_last as u8) << 6 | (self.moved as u8) << 5
    }

    /// Snapshot the current state for a ReadMouse and rewind the read stream;
    /// a read is the boundary that resets "moved" and the last-button state.
    fn latch(&mut self) {
        self.latched_x = self.x as u16;
        self.latched_y = self.y as u16;
        self.latched_status = self.status();
        self.latched_mode = self.mode;
        self.read_index = 0;
        self.button_last = self.button;
        self.moved = false;
    }

    /// The next byte of the latched snapshot: Xlo, Xhi, Ylo, Yhi, status, mode.
    fn next_read_byte(&mut self) -> u8 {
        let bytes = [
            self.latched_x as u8,
            (self.latched_x >> 8) as u8,
            self.latched_y as u8,
            (self.latched_y >> 8) as u8,
            self.latched_status,
            self.latched_mode,
        ];
        let b = bytes.get(self.read_index).copied().unwrap_or(0);
        self.read_index = (self.read_index + 1).min(bytes.len());
        b
    }

    /// The 16-bit parameter in temp ports 0-1 (X) or 2-3 (Y), sign-extended.
    fn temp_x(&self) -> i32 {
        i16::from_le_bytes([self.temp[0], self.temp[1]]) as i32
    }
    fn temp_y(&self) -> i32 {
        i16::from_le_bytes([self.temp[2], self.temp[3]]) as i32
    }

    /// Dispatch a command byte written to port 8.
    fn command(&mut self, cmd: u8) {
        match cmd {
            0 => {
                // SetPos: force the position (clamped) from the temps.
                self.x = self.temp_x().clamp(self.clamp_x.0, self.clamp_x.1);
                self.y = self.temp_y().clamp(self.clamp_y.0, self.clamp_y.1);
            }
            1 => {
                // InitMouse: defaults.
                *self = Mou::new();
            }
            2 => {
                self.x = 0;
                self.y = 0;
            }
            3 => {
                // HomeMouse: to the clamp minimum.
                self.x = self.clamp_x.0;
                self.y = self.clamp_y.0;
            }
            4 => {
                // ClampX: min in the X temps, max in the Y temps.
                self.clamp_x = (self.temp_x(), self.temp_y());
                self.x = self.x.clamp(self.clamp_x.0, self.clamp_x.1);
            }
            5 => {
                self.clamp_y = (self.temp_x(), self.temp_y());
                self.y = self.y.clamp(self.clamp_y.0, self.clamp_y.1);
            }
            _ => {}
        }
    }

    /// Move the emulated mouse to an absolute `(x, y)` within the clamp window
    /// and set the button — the RFB path (a mapped framebuffer pixel) and a
    /// test aid. Sets "moved" when the clamped position changes.
    pub fn set_host(&mut self, x: i32, y: i32, button: bool) {
        self.set_host_position(x, y);
        self.button = button;
    }

    /// Integrate a relative movement (host deltas) within the clamp window —
    /// the SDL captured/relative path, which is what the hardware does.
    pub fn move_by(&mut self, dx: i32, dy: i32) {
        self.set_host_position(self.x + dx, self.y + dy);
    }

    fn set_host_position(&mut self, x: i32, y: i32) {
        let nx = x.clamp(self.clamp_x.0, self.clamp_x.1);
        let ny = y.clamp(self.clamp_y.0, self.clamp_y.1);
        if nx != self.x || ny != self.y {
            self.moved = true;
            if self.mode & (MODE_ON | INT_MOVE) == MODE_ON | INT_MOVE {
                self.irq_source |= INT_MOVE;
            }
        }
        self.x = nx;
        self.y = ny;
    }

    /// Press or release the host button. A press edge raises the button
    /// interrupt when the mode enables it.
    pub fn set_button(&mut self, down: bool) {
        if down && !self.button && self.mode & (MODE_ON | INT_BUTTON) == MODE_ON | INT_BUTTON {
            self.irq_source |= INT_BUTTON;
        }
        self.button = down;
    }

    /// A once-per-frame vertical-blank tick (plans/20260721-01 M4): raises the
    /// VBL interrupt source when the mode enables it (mouse on + VBL bit).
    pub fn vbl_tick(&mut self) {
        if self.mode & (MODE_ON | INT_VBL) == MODE_ON | INT_VBL {
            self.irq_source |= INT_VBL;
        }
    }

    /// Whether the card is asserting its maskable IRQ — some enabled source is
    /// pending, until ServeMouse (a read of port 9) clears it.
    pub fn irq_asserted(&self) -> bool {
        self.irq_source != 0
    }

    /// The clamp window `(min_x, max_x, min_y, max_y)`, for mapping an
    /// absolute host pointer (RFB) into it.
    pub fn clamp(&self) -> (i32, i32, i32, i32) {
        (
            self.clamp_x.0,
            self.clamp_x.1,
            self.clamp_y.0,
            self.clamp_y.1,
        )
    }
}

impl Default for Mou {
    fn default() -> Mou {
        Mou::new()
    }
}

impl Device for Mou {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        match addr & 0x0f {
            0x0 => self.status(),
            0x1 => self.next_read_byte(),
            0x2 => self.mode,
            0x9 => std::mem::take(&mut self.irq_source),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, b: u8, _cycles: u64) {
        match addr & 0x0f {
            0x0 => self.latch(),
            0x2 => self.mode = b,
            0x4..=0x7 => self.temp[(addr & 0x0f) as usize - 4] = b,
            0x8 => self.command(b),
            _ => {}
        }
    }
}

/// The full mouse state round-trips (notes/STATE.md): position, clamps,
/// button/moved, mode, and the in-flight read snapshot so a suspended
/// ReadMouse resumes correctly.
impl ewm_core::state::Persist for Mou {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        for v in [
            self.x,
            self.y,
            self.clamp_x.0,
            self.clamp_x.1,
            self.clamp_y.0,
            self.clamp_y.1,
        ] {
            w.put_u32(v as u32);
        }
        w.put_u8((self.button as u8) | (self.button_last as u8) << 1 | (self.moved as u8) << 2);
        w.put_u8(self.mode);
        w.put_u16(self.latched_x);
        w.put_u16(self.latched_y);
        w.put_u8(self.latched_status);
        w.put_u8(self.latched_mode);
        w.put_u16(self.read_index as u16);
        w.put_bytes(&self.temp);
        w.put_u8(self.irq_source);
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.x = r.get_u32()? as i32;
        self.y = r.get_u32()? as i32;
        self.clamp_x = (r.get_u32()? as i32, r.get_u32()? as i32);
        self.clamp_y = (r.get_u32()? as i32, r.get_u32()? as i32);
        let flags = r.get_u8()?;
        self.button = flags & 1 != 0;
        self.button_last = flags & 2 != 0;
        self.moved = flags & 4 != 0;
        self.mode = r.get_u8()?;
        self.latched_x = r.get_u16()?;
        self.latched_y = r.get_u16()?;
        self.latched_status = r.get_u8()?;
        self.latched_mode = r.get_u8()?;
        self.read_index = (r.get_u16()? as usize).min(6);
        self.temp.copy_from_slice(r.get_bytes(4)?);
        self.irq_source = r.get_u8()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated slot-4 firmware, pinned byte-for-byte (like `clk.rs`).
    #[test]
    fn mouse_rom_slot4_is_golden() {
        let rom = mouse_rom(4);
        // Identification the slot scan / driver probe reads.
        assert_ne!(rom[0x01], 0x20, "must not look like a Disk II");
        assert_eq!(rom[0x05], 0x38);
        assert_eq!(rom[0x07], 0x18);
        assert_eq!(rom[0x0b], 0x01);
        assert_eq!(rom[0x0c], 0x20);
        assert_eq!(rom[0xfb], 0xd6, "AppleMouse ID");
        // The offset table points into the routine area, each at a real
        // routine start (first byte an opcode we emitted, never $00).
        for (i, &off) in rom[0x12..0x1a].iter().enumerate() {
            assert!(
                off >= 0x1a,
                "entry {i} offset ${off:02x} is in the routine area"
            );
            assert_ne!(
                rom[off as usize], 0x00,
                "entry {i} starts at a real routine"
            );
        }
        // ReadMouse (table entry 2) latches then streams into the slot-4
        // screen holes; spot-check its first instructions.
        let read = rom[0x14] as usize; // $Cn12 + 2
        assert_eq!(
            &rom[read..read + 3],
            &[0x8d, 0xc0, 0xc0],
            "STA $C0C0 (latch)"
        );
        assert_eq!(
            &rom[read + 3..read + 9],
            &[0xad, 0xc1, 0xc0, 0x8d, 0x7c, 0x04],
            "LDA $C0C1; STA $047C"
        );
    }

    /// Slot patches the DEVSEL and screen-hole operands (like `clk.rs`).
    #[test]
    fn mouse_rom_patches_slot_operands() {
        let s4 = mouse_rom(4);
        let s5 = mouse_rom(5);
        assert_ne!(s4, s5, "different slots patch different operands");
        // Slot 5's DEVSEL base is $C0D0 (0x80 + 5*16); ReadMouse latches there.
        let read5 = s5[0x14] as usize;
        assert_eq!(
            &s5[read5..read5 + 3],
            &[0x8d, 0xd0, 0xc0],
            "slot 5 latches $C0D0"
        );
    }

    // The device is driven directly over its DEVSEL ports — the "direct
    // soft-switch drive" the M2 gate allows, standing in for the firmware.
    fn write(m: &mut Mou, off: u16, b: u8) {
        m.write(0xc0c0 + off, b, 0);
    }
    fn read(m: &mut Mou, off: u16) -> u8 {
        m.read(0xc0c0 + off, 0)
    }
    /// InitMouse → ClampMouse → PosMouse → ReadMouse, driven over the ports.
    fn set_pos(m: &mut Mou, x: i16, y: i16, cmd: u8) {
        let [xl, xh] = x.to_le_bytes();
        let [yl, yh] = y.to_le_bytes();
        write(m, 4, xl);
        write(m, 5, xh);
        write(m, 6, yl);
        write(m, 7, yh);
        write(m, 8, cmd);
    }
    /// Latch, then pull the six-byte snapshot the firmware copies to holes.
    fn read_snapshot(m: &mut Mou) -> (u16, u16, u8, u8) {
        write(m, 0, 0); // latch
        let xl = read(m, 1);
        let xh = read(m, 1);
        let yl = read(m, 1);
        let yh = read(m, 1);
        let status = read(m, 1);
        let mode = read(m, 1);
        (
            u16::from_le_bytes([xl, xh]),
            u16::from_le_bytes([yl, yh]),
            status,
            mode,
        )
    }

    #[test]
    fn init_clamp_pos_read_deposits_clamped_values() {
        let mut m = Mou::new();
        write(&mut m, 8, 1); // InitMouse: clamp 0..=1023, mouse off
        set_pos(&mut m, 100, 700, 4); // ClampX min=100 max=700
        set_pos(&mut m, 200, 500, 5); // ClampY min=200 max=500
        write(&mut m, 2, 0x01); // SetMouse: mode = mouse on
        set_pos(&mut m, 400, 400, 0); // PosMouse inside the window
        let (x, y, _status, mode) = read_snapshot(&mut m);
        assert_eq!((x, y), (400, 400), "position lands inside the clamp");
        assert_eq!(mode, 0x01, "mode reads back");

        // Positions outside the clamp are pinned at the bounds.
        set_pos(&mut m, 9999, -9999, 0);
        let (x, y, _, _) = read_snapshot(&mut m);
        assert_eq!((x, y), (700, 200), "clamped to (maxX, minY)");
    }

    #[test]
    fn status_tracks_button_and_movement() {
        let mut m = Mou::new();
        // A fresh read: no button, no movement.
        let (_, _, status, _) = read_snapshot(&mut m);
        assert_eq!(status, 0);

        // Press and move, then read: button-now and moved set.
        m.set_host(300, 300, true);
        let (_, _, status, _) = read_snapshot(&mut m);
        assert_eq!(status & 0x80, 0x80, "button down now");
        assert_eq!(status & 0x20, 0x20, "moved since last read");

        // Hold the button, don't move: button-now and button-last set, moved
        // clear (the previous read reset it).
        let (_, _, status, _) = read_snapshot(&mut m);
        assert_eq!(status & 0xc0, 0xc0, "button now + at last read");
        assert_eq!(status & 0x20, 0, "no movement since the last read");
    }

    #[test]
    fn home_moves_to_clamp_minimum() {
        let mut m = Mou::new();
        set_pos(&mut m, 100, 200, 4); // ClampX 100..200
        set_pos(&mut m, 300, 400, 5); // ClampY 300..400
        write(&mut m, 8, 3); // HomeMouse
        let (x, y, _, _) = read_snapshot(&mut m);
        assert_eq!((x, y), (100, 300), "home is the clamp minimum");
    }

    #[test]
    fn vbl_interrupt_is_mode_gated_and_serve_clears_it() {
        // M4: a VBL tick raises the IRQ only when the mode enables it (mouse
        // on + VBL bit); ServeMouse (a read of port 9) reports the source and
        // de-asserts.
        let mut m = Mou::new();
        m.vbl_tick();
        assert!(!m.irq_asserted(), "no interrupt without the enable bits");

        write(&mut m, 2, 0x09); // SetMouse: mouse on + VBL interrupt
        m.vbl_tick();
        assert!(m.irq_asserted(), "VBL raises the line when enabled");
        assert_eq!(read(&mut m, 9), 0x08, "ServeMouse source is VBL");
        assert!(!m.irq_asserted(), "ServeMouse cleared it");

        // A movement raises the line when movement interrupts are on.
        write(&mut m, 2, 0x03); // mouse on + movement interrupt
        m.set_host(400, 400, false);
        assert!(m.irq_asserted(), "movement raises the line");
        assert_eq!(read(&mut m, 9) & 0x02, 0x02, "source is movement");
    }
}
