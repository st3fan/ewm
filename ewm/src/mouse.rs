//! The AppleMouse II card (`{"card": "mouse"}`, slot 4 by default, any slot
//! 1–7), modelled as the **real hardware**: a 6520 PIA at the slot DEVSEL, a
//! 6805 microcontroller (the "mouse brain"), and the card's own 2 KB
//! `342-0270-C` ROM banked into the 256-byte `$Cn00` slot area. Unlike the
//! synthetic firmware it replaces (plans/20260721-01), this drives *any*
//! mouse software — including the //e path MousePaint uses, which talks to
//! the PIA directly rather than the card's entry points.
//!
//! Ported from the MIT `oliverschmidt/mouse-interface` reference
//! (`PIA6520.c` — the PIA; `MouseInterfaceCard.c` — the 6805 controller and
//! the fully documented 6502↔6805 handshake); the ROM is from
//! `freitz85/AppleIIMouse`. See notes/MOUSE.md and
//! plans/20260721-03-mouse-pia-hardware.md.
//!
//! ## The pieces
//!
//! - **PIA (`Pia`)** at DEVSEL offsets 0-3 (`PRA`/`CRA`/`PRB`/`CRB`); the low
//!   two address bits decode the four registers. `CRx bit 2` selects the data
//!   port vs. its direction register. Port A carries the data byte to/from
//!   the 6805; port B carries the handshake, the ROM bank select, and a sync
//!   bit.
//! - **6805 (`Ctl`)** — the mouse state (position, clamp window, buttons,
//!   mode, interrupt state) and the command engine. The 6502 sends 1-5 byte
//!   commands over port A, gated by the port-B handshake; some commands reply.
//! - **Banked ROM** — the 2 KB ROM as eight 256-byte pages; PIA port B bits
//!   1-3 select which page is visible at `$Cn00`. The page-switch code sits at
//!   `$xx70` of every page, so the firmware can flip banks mid-routine.
//!
//! ## Port B bits (`MouseInterfaceCard.c`)
//!
//! bit0 sync latch, bits1-3 ROM page (A8-A10), bit4 RDACK, bit5 WRREQUEST,
//! bit6 RDREADY, bit7 WRACK. The ROM sets port B DDR to `0x3E` (bits 1-5
//! output). The 6805 drives the slot IRQ line directly — the PIA's own IRQ is
//! unused.
//!
//! ## The handshake (no timeouts — a wrong step hangs the firmware)
//!
//! - **Write (6502→6805):** 6502 sets WRREQUEST, waits WRACK; the 6805
//!   latches port A and sets WRACK; 6502 clears WRREQUEST, waits ¬WRACK; the
//!   6805 clears WRACK.
//! - **Read (6805→6502):** the 6805 sets RDREADY with a data byte on port A;
//!   6502 reads it, sets RDACK, waits ¬RDREADY; the 6805 clears RDREADY; 6502
//!   clears RDACK.
//!
//! EWM has no second core, so the 6805's run loop is modelled as a state
//! machine advanced to a fixpoint after every PIA write (the write is what
//! moves port B), plus the once-per-frame VBL tick.

use ewm_core::mem::Device;

/// The card ROM: 2 KB (eight 256-byte pages), banked into `$Cn00` by PIA port
/// B bits 1-3. Apple part `342-0270-C`; sha1
/// `3a9d881a8a8d30f55b9719aceebbcf717f829d6f` (freitz85/AppleIIMouse), pinned
/// by `mouse_rom_is_the_committed_image`.
static MOUSE_ROM: &[u8; 2048] =
    include_bytes!("../../roms/342-0270-C — AppleMouse II Interface Card (2716).bin");

// PIA port B handshake bits.
const PB_RDACK: u8 = 0x10;
const PB_WRREQUEST: u8 = 0x20;
const PB_RDREADY: u8 = 0x40;
const PB_WRACK: u8 = 0x80;

// Commands (top nibble of the first byte).
const CMD_SETMOUSE: u8 = 0x00;
const CMD_READMOUSE: u8 = 0x10;
const CMD_SERVEMOUSE: u8 = 0x20;
const CMD_CLEARMOUSE: u8 = 0x30;
const CMD_POSMOUSE: u8 = 0x40;
const CMD_INITMOUSE: u8 = 0x50;
const CMD_CLAMPMOUSE: u8 = 0x60;
const CMD_HOMEMOUSE: u8 = 0x70;
const CMD_TIMEMOUSE: u8 = 0x90;
const CMD_A0: u8 = 0xa0;
const CMD_RDMEMMOUSE: u8 = 0xf0;

// Status byte bits.
const STATUS_WAS_BUTTON1: u8 = 1 << 0;
const STATUS_IRQ_MOVEMENT: u8 = 1 << 1;
const STATUS_IRQ_BUTTON: u8 = 1 << 2;
const STATUS_IRQ_VBL: u8 = 1 << 3;
const STATUS_IS_BUTTON1: u8 = 1 << 4;
const STATUS_MOVED: u8 = 1 << 5;
const STATUS_WAS_BUTTON0: u8 = 1 << 6;
const STATUS_IS_BUTTON0: u8 = 1 << 7;

/// The three interrupt sources ServeMouse reports and clears.
const IRQ_SOURCES: u8 = STATUS_IRQ_VBL | STATUS_IRQ_MOVEMENT | STATUS_IRQ_BUTTON;

// Operating-mode bits (SetMouse). A movement/button interrupt requires the
// mouse to be enabled too; a VBL interrupt does not.
const MODE_ENABLED: u8 = 1 << 0;
const MODE_MOVED_IRQ: u8 = (1 << 1) | MODE_ENABLED;
const MODE_BUTTON_IRQ: u8 = (1 << 2) | MODE_ENABLED;
const MODE_VBL_IRQ: u8 = 1 << 3;

/// VBL period in CPU cycles: 60 Hz (US) and 50 Hz (EU). Kept for TIMEMOUSE;
/// the frame-driven `vbl_tick` uses the host frame rate, not this counter.
const US_60HZ_CYCLES: u16 = 17030;
const EU_50HZ_CYCLES: u16 = 20280;

/// The default clamp maximum on both axes (`0..=1023`), the firmware default a
/// program overrides with ClampMouse.
const CLAMP_MAX: i16 = 1023;

/// The Rockwell 6520 PIA (port of `PIA6520.c`). Two 8-bit ports, each with an
/// output register (`or*`), a direction register (`ddr*`), and an input latch
/// (`i*`) the 6805 drives. The physical port value is
/// `(OR & DDR) | (IN & ~DDR)`. The PIA's own IRQ machinery is unused on this
/// card, so it is not modelled.
#[derive(Default)]
struct Pia {
    ddra: u8,
    ddrb: u8,
    ora: u8,
    orb: u8,
    cra: u8,
    crb: u8,
    /// Input latches, driven by the 6805 side.
    ia: u8,
    ib: u8,
}

impl Pia {
    /// The physical port A value (what the 6805 reads / the 6502 sees).
    fn port_a(&self) -> u8 {
        (self.ora & self.ddra) | (self.ia & !self.ddra)
    }

    /// The physical port B value: handshake + ROM bank + sync.
    fn port_b(&self) -> u8 {
        (self.orb & self.ddrb) | (self.ib & !self.ddrb)
    }

    /// Read register `reg` (0-3). Data-vs-direction is chosen by `CRx bit 2`.
    fn read(&self, reg: u8) -> u8 {
        match reg & 0x03 {
            0 => {
                if self.cra & 0x04 != 0 {
                    self.port_a()
                } else {
                    self.ddra
                }
            }
            1 => self.cra,
            2 => {
                if self.crb & 0x04 != 0 {
                    self.port_b()
                } else {
                    self.ddrb
                }
            }
            _ => self.crb,
        }
    }

    /// Write register `reg` (0-3). The control registers keep only bits 0-5
    /// (the CA/CB interrupt bits are read-only and unused here).
    fn write(&mut self, reg: u8, data: u8) {
        match reg & 0x03 {
            0 => {
                if self.cra & 0x04 != 0 {
                    self.ora = data;
                } else {
                    self.ddra = data;
                }
            }
            1 => self.cra = data & 0x3f,
            2 => {
                if self.crb & 0x04 != 0 {
                    self.orb = data;
                } else {
                    self.ddrb = data;
                }
            }
            _ => self.crb = data & 0x3f,
        }
    }
}

/// The 6805 controller state (`TA2Mouse` in `MouseInterfaceCard.c`): the
/// command engine's buffers and handshake cursors, plus the mouse's current
/// and last-read position/buttons, the clamp window, the operating mode, and
/// the pending interrupt state.
struct Ctl {
    command: u8,
    read_buffer: [u8; 8],
    write_buffer: [u8; 8],
    /// Bytes still available to read back / parameter bytes still expected.
    read_pos: u8,
    write_pos: u8,
    /// Port B at the previous run, for WRREQUEST edge detection.
    last_port_b: u8,
    inter_vbl_cycles: u16,
    operating_mode: u8,
    /// The interrupt/status accumulator (the ReadMouse status byte source).
    int_state: u8,
    cur_x: i16,
    cur_y: i16,
    cur_b0: bool,
    cur_b1: bool,
    last_x: i16,
    last_y: i16,
    last_b0: bool,
    last_b1: bool,
    min_x: i16,
    min_y: i16,
    max_x: i16,
    max_y: i16,
}

impl Default for Ctl {
    fn default() -> Ctl {
        Ctl {
            command: 0,
            read_buffer: [0; 8],
            write_buffer: [0; 8],
            read_pos: 0,
            write_pos: 0,
            last_port_b: 0,
            inter_vbl_cycles: US_60HZ_CYCLES,
            operating_mode: 0,
            int_state: 0,
            cur_x: 0,
            cur_y: 0,
            cur_b0: false,
            cur_b1: false,
            last_x: 0,
            last_y: 0,
            last_b0: false,
            last_b1: false,
            min_x: 0,
            min_y: 0,
            max_x: CLAMP_MAX,
            max_y: CLAMP_MAX,
        }
    }
}

/// The AppleMouse card: the PIA + the 6805 controller, mapped both to the
/// slot's 16-byte DEVSEL range (the PIA) and to `$Cn00` (the banked ROM). The
/// `slot` is only used to pick the ROM/DEVSEL decode; the same device works in
/// any slot.
pub struct Mou {
    pia: Pia,
    ctl: Ctl,
    #[allow(dead_code)]
    slot: u8,
}

impl Mou {
    pub fn new(slot: u8) -> Mou {
        Mou {
            pia: Pia::default(),
            ctl: Ctl::default(),
            slot,
        }
    }

    // ---- the banked $Cn00 ROM ----

    /// The ROM page currently selected by PIA port B bits 1-3.
    fn rom_bank(&self) -> usize {
        ((self.pia.port_b() >> 1) & 0x07) as usize
    }

    /// The ROM byte at `$Cn00 + offset`, from the selected page.
    fn rom_byte(&self, offset: u8) -> u8 {
        MOUSE_ROM[self.rom_bank() * 256 + offset as usize]
    }

    // ---- the 6805 run loop (port of mouseControllerRun, minus VBL) ----

    /// Advance the controller until its inputs to the 6502 (port A / port B
    /// latches) stop changing — the fixpoint the reference's tight run loop
    /// would reach between two 6502 bus accesses. Called after every PIA
    /// write, the only thing that moves port B.
    fn advance(&mut self) {
        for _ in 0..8 {
            let (ia, ib) = (self.pia.ia, self.pia.ib);
            self.run_once();
            if self.pia.ia == ia && self.pia.ib == ib {
                break;
            }
        }
    }

    fn run_once(&mut self) {
        let port_b = self.pia.port_b();
        // A change on WRREQUEST (either edge) drives the write side.
        if (port_b ^ self.ctl.last_port_b) & PB_WRREQUEST != 0 {
            self.controller_write(port_b);
        }
        self.controller_read(port_b);
        self.ctl.last_port_b = port_b;
    }

    /// The write side: on WRREQUEST high, latch the data byte and acknowledge;
    /// on WRREQUEST low, drop the acknowledge.
    fn controller_write(&mut self, port_b: u8) {
        if port_b & PB_WRREQUEST != 0 {
            self.ctl.read_pos = 0; // any un-read reply is discarded
            self.accept_data();
            self.pia.ib = (self.pia.ib & !PB_RDREADY) | PB_WRACK;
        } else if self.pia.ib & PB_WRACK != 0 {
            self.pia.ib &= !PB_WRACK;
        }
    }

    /// The read side: on RDACK, retire the byte just read and drop RDREADY;
    /// otherwise, when nothing is being written, present the next reply byte
    /// and raise RDREADY. RDREADY is offered even with no pending reply (the
    /// reference's anti-hang guard: an unexpected read must not stall forever).
    fn controller_read(&mut self, port_b: u8) {
        if port_b & PB_RDACK != 0 {
            if self.pia.ib & PB_RDREADY != 0 {
                self.ctl.read_pos = self.ctl.read_pos.saturating_sub(1);
                self.pia.ib &= !PB_RDREADY;
            }
        } else if port_b & (PB_WRACK | PB_WRREQUEST) == 0 && self.pia.ib & PB_RDREADY == 0 {
            let byte = if self.ctl.read_pos > 0 {
                self.ctl.read_buffer[self.ctl.read_pos as usize - 1]
            } else {
                0
            };
            self.pia.ia = byte;
            self.pia.ib |= PB_RDREADY;
        }
    }

    /// Latch a byte written by the 6502: either the next parameter of the
    /// command in flight, or the first byte of a new command (whose parameter
    /// count then determines how many more bytes to expect). Runs the command
    /// once every expected byte has arrived.
    fn accept_data(&mut self) {
        if self.ctl.write_pos != 0 {
            self.ctl.write_pos -= 1;
            self.ctl.write_buffer[self.ctl.write_pos as usize] = self.pia.port_a();
        } else {
            self.ctl.command = self.pia.port_a();
            self.ctl.write_pos = match self.ctl.command & 0xf0 {
                CMD_POSMOUSE | CMD_CLAMPMOUSE => 4,
                CMD_A0 => 1,
                CMD_RDMEMMOUSE => 2,
                CMD_TIMEMOUSE => match self.ctl.command & 0x0c {
                    0x4 => 2,
                    0x8 => 1,
                    0xc => 3,
                    _ => 0,
                },
                _ => 0,
            };
        }
        if self.ctl.write_pos == 0 {
            self.command();
        }
    }

    // ---- commands (each a port of mouseCommand*) ----

    fn command(&mut self) {
        match self.ctl.command & 0xf0 {
            CMD_SETMOUSE => self.ctl.operating_mode = self.ctl.command & 0x0f,
            CMD_READMOUSE => self.command_read(),
            CMD_SERVEMOUSE => self.command_serve(),
            CMD_CLEARMOUSE => {
                self.ctl.cur_x = 0;
                self.ctl.cur_y = 0;
            }
            CMD_POSMOUSE => self.command_pos(),
            CMD_INITMOUSE => self.command_init(),
            CMD_CLAMPMOUSE => self.command_clamp(),
            CMD_HOMEMOUSE => self.command_home(),
            CMD_TIMEMOUSE => {
                self.ctl.inter_vbl_cycles = if self.ctl.command & 0x01 != 0 {
                    EU_50HZ_CYCLES
                } else {
                    US_60HZ_CYCLES
                };
            }
            CMD_RDMEMMOUSE => self.command_read_mem(),
            _ => {} // $8n/$An/$Bn/$Cn: unimplemented on the real 6805 too
        }
    }

    fn command_read(&mut self) {
        let mut status = self.ctl.int_state & STATUS_MOVED;
        if self.ctl.last_b0 {
            status |= STATUS_WAS_BUTTON0;
        }
        if self.ctl.last_b1 {
            status |= STATUS_WAS_BUTTON1;
        }
        if self.ctl.cur_b0 {
            status |= STATUS_IS_BUTTON0;
        }
        if self.ctl.cur_b1 {
            status |= STATUS_IS_BUTTON1;
        }
        let x = self.ctl.cur_x as u16;
        let y = self.ctl.cur_y as u16;
        self.ctl.read_buffer[4] = x as u8;
        self.ctl.read_buffer[3] = (x >> 8) as u8;
        self.ctl.read_buffer[2] = y as u8;
        self.ctl.read_buffer[1] = (y >> 8) as u8;
        self.ctl.read_buffer[0] = status;
        self.ctl.int_state = status & !STATUS_MOVED;
        self.ctl.last_x = self.ctl.cur_x;
        self.ctl.last_y = self.ctl.cur_y;
        self.ctl.last_b0 = self.ctl.cur_b0;
        self.ctl.last_b1 = self.ctl.cur_b1;
        self.ctl.read_pos = 5;
    }

    fn command_serve(&mut self) {
        // Report the status without the "moved" bit, then clear the interrupt
        // sources — de-asserting the IRQ line (see `irq_asserted`).
        self.ctl.read_buffer[0] = self.ctl.int_state & !STATUS_MOVED;
        self.ctl.read_pos = 1;
        self.ctl.int_state &= !IRQ_SOURCES;
    }

    fn command_pos(&mut self) {
        self.ctl.cur_x = le16(self.ctl.write_buffer[3], self.ctl.write_buffer[2]);
        self.ctl.cur_y = le16(self.ctl.write_buffer[1], self.ctl.write_buffer[0]);
        self.clamp_xy();
        self.ctl.last_x = self.ctl.cur_x;
        self.ctl.last_y = self.ctl.cur_y;
    }

    fn command_init(&mut self) {
        self.ctl.min_x = 0;
        self.ctl.min_y = 0;
        self.ctl.max_x = CLAMP_MAX;
        self.ctl.max_y = CLAMP_MAX;
        self.command_home();
        self.ctl.int_state &= !IRQ_SOURCES;
    }

    fn command_home(&mut self) {
        self.ctl.cur_x = self.ctl.min_x;
        self.ctl.last_x = self.ctl.min_x;
        self.ctl.cur_y = self.ctl.min_y;
        self.ctl.last_y = self.ctl.min_y;
    }

    fn command_clamp(&mut self) {
        let mut min = le16(self.ctl.write_buffer[3], self.ctl.write_buffer[1]);
        let mut max = le16(self.ctl.write_buffer[2], self.ctl.write_buffer[0]);
        if min > max {
            // The reference's degenerate-range fixup: average, floor at 0.
            let t = (max as i32 as u32).wrapping_add(min as i32 as u32);
            max = (t >> 1) as u16 as i16;
            min = 0;
        }
        if self.ctl.command & 0x01 != 0 {
            self.ctl.min_y = min;
            self.ctl.max_y = max;
        } else {
            self.ctl.min_x = min;
            self.ctl.max_x = max;
        }
        self.clamp_xy();
    }

    fn command_read_mem(&mut self) {
        // Documented GETCLAMP workaround (Apple II Technical Note, Mouse #7):
        // read the clamp boundaries out of the controller's memory.
        let address = le16(self.ctl.write_buffer[1], self.ctl.write_buffer[0]) as u16;
        self.ctl.read_buffer[0] = match address {
            0x47 => (self.ctl.min_x as u16 >> 8) as u8,
            0x48 => (self.ctl.min_y as u16 >> 8) as u8,
            0x49 => self.ctl.min_x as u8,
            0x4a => self.ctl.min_y as u8,
            0x4b => (self.ctl.max_x as u16 >> 8) as u8,
            0x4c => (self.ctl.max_y as u16 >> 8) as u8,
            0x4d => self.ctl.max_x as u8,
            0x4e => self.ctl.max_y as u8,
            _ => 0,
        };
        self.ctl.read_pos = 1;
    }

    fn clamp_xy(&mut self) {
        self.ctl.cur_x = self.ctl.cur_x.clamp(self.ctl.min_x, self.ctl.max_x);
        self.ctl.cur_y = self.ctl.cur_y.clamp(self.ctl.min_y, self.ctl.max_y);
    }

    // ---- host input (port of mouseControllerMoveXY / UpdateButton) ----

    /// Integrate a relative host movement within the clamp window — the SDL
    /// captured/relative path. Sets "moved" and raises the movement interrupt
    /// when enabled.
    pub fn move_by(&mut self, dx: i32, dy: i32) {
        let x = self.ctl.cur_x as i32 + dx;
        let y = self.ctl.cur_y as i32 + dy;
        self.set_position(x, y);
    }

    /// Place the mouse at an absolute position (already in mouse coordinates)
    /// within the clamp window — the RFB/SDL absolute-mapped path. Sets "moved"
    /// and raises the movement interrupt when enabled.
    pub fn set_position(&mut self, x: i32, y: i32) {
        let nx = x.clamp(self.ctl.min_x as i32, self.ctl.max_x as i32) as i16;
        let ny = y.clamp(self.ctl.min_y as i32, self.ctl.max_y as i32) as i16;
        if nx != self.ctl.cur_x || ny != self.ctl.cur_y {
            self.ctl.int_state |= STATUS_MOVED;
            if self.ctl.operating_mode & MODE_MOVED_IRQ == MODE_MOVED_IRQ {
                self.ctl.int_state |= STATUS_IRQ_MOVEMENT;
            }
        }
        self.ctl.cur_x = nx;
        self.ctl.cur_y = ny;
    }

    /// Press or release the host button (button 0). Raises the button
    /// interrupt when enabled.
    pub fn set_button(&mut self, down: bool) {
        self.ctl.cur_b0 = down;
        if self.ctl.operating_mode & MODE_BUTTON_IRQ == MODE_BUTTON_IRQ {
            self.ctl.int_state |= STATUS_IRQ_BUTTON;
        }
    }

    /// The once-per-frame vertical-blank tick: raises the VBL interrupt when
    /// the VBL-interrupt mode bit is set (active even with the mouse "off").
    pub fn vbl_tick(&mut self) {
        if self.ctl.operating_mode & MODE_VBL_IRQ == MODE_VBL_IRQ {
            self.ctl.int_state |= STATUS_IRQ_VBL;
        }
    }

    /// Whether the card is asserting its slot IRQ — some interrupt source is
    /// pending, until ServeMouse clears it.
    pub fn irq_asserted(&self) -> bool {
        self.ctl.int_state & IRQ_SOURCES != 0
    }

    /// The clamp window `(min_x, max_x, min_y, max_y)`, for mapping an absolute
    /// host pointer into it (`Two::feed_mouse_pixel`).
    pub fn clamp(&self) -> (i32, i32, i32, i32) {
        (
            self.ctl.min_x as i32,
            self.ctl.max_x as i32,
            self.ctl.min_y as i32,
            self.ctl.max_y as i32,
        )
    }

    /// Introspection: the current mouse position (the 6805's `Current` X/Y).
    pub fn position(&self) -> (i16, i16) {
        (self.ctl.cur_x, self.ctl.cur_y)
    }

    /// Introspection: the live status byte (button now/at-last-read, moved).
    pub fn status_byte(&self) -> u8 {
        let mut status = self.ctl.int_state & STATUS_MOVED;
        if self.ctl.last_b0 {
            status |= STATUS_WAS_BUTTON0;
        }
        if self.ctl.last_b1 {
            status |= STATUS_WAS_BUTTON1;
        }
        if self.ctl.cur_b0 {
            status |= STATUS_IS_BUTTON0;
        }
        if self.ctl.cur_b1 {
            status |= STATUS_IS_BUTTON1;
        }
        status
    }

    /// Set the operating mode directly, without the SetMouse handshake — a
    /// test hook for the interrupt-plumbing tests (the handshake path is
    /// exercised by the firmware-driven tests).
    #[cfg(test)]
    pub fn set_operating_mode(&mut self, mode: u8) {
        self.ctl.operating_mode = mode;
    }

    /// Run a command directly (no handshake) — a test hook (e.g. ServeMouse to
    /// clear the interrupt in the IRQ-plumbing tests).
    #[cfg(test)]
    pub fn run_command(&mut self, command: u8) {
        self.ctl.command = command;
        self.command();
    }
}

/// Little-endian 16-bit assembly (`lo | hi<<8`) as a signed 16-bit value.
fn le16(lo: u8, hi: u8) -> i16 {
    ((lo as u16) | ((hi as u16) << 8)) as i16
}

impl Device for Mou {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        if addr >> 8 == 0xc0 {
            // The slot DEVSEL: the PIA. Two address bits decode its registers.
            self.pia.read((addr & 0x03) as u8)
        } else {
            // The banked $Cn00 slot ROM.
            self.rom_byte(addr as u8)
        }
    }

    fn write(&mut self, addr: u16, b: u8, _cycles: u64) {
        if addr >> 8 == 0xc0 {
            self.pia.write((addr & 0x03) as u8, b);
            self.advance(); // let the 6805 react to the new port B
        }
        // Writes to the $Cn00 ROM region are swallowed.
    }
}

/// The full card state round-trips (notes/STATE.md): the PIA registers and the
/// 6805 controller's buffers, cursors, position, clamp, and interrupt state,
/// so a suspended handshake or in-flight read resumes correctly.
impl ewm_core::state::Persist for Mou {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        let p = &self.pia;
        for v in [p.ddra, p.ddrb, p.ora, p.orb, p.cra, p.crb, p.ia, p.ib] {
            w.put_u8(v);
        }
        let c = &self.ctl;
        w.put_u8(c.command);
        w.put_bytes(&c.read_buffer);
        w.put_bytes(&c.write_buffer);
        w.put_u8(c.read_pos);
        w.put_u8(c.write_pos);
        w.put_u8(c.last_port_b);
        w.put_u16(c.inter_vbl_cycles);
        w.put_u8(c.operating_mode);
        w.put_u8(c.int_state);
        for v in [
            c.cur_x, c.cur_y, c.last_x, c.last_y, c.min_x, c.min_y, c.max_x, c.max_y,
        ] {
            w.put_u16(v as u16);
        }
        w.put_u8(
            (c.cur_b0 as u8)
                | (c.cur_b1 as u8) << 1
                | (c.last_b0 as u8) << 2
                | (c.last_b1 as u8) << 3,
        );
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        let p = &mut self.pia;
        p.ddra = r.get_u8()?;
        p.ddrb = r.get_u8()?;
        p.ora = r.get_u8()?;
        p.orb = r.get_u8()?;
        p.cra = r.get_u8()?;
        p.crb = r.get_u8()?;
        p.ia = r.get_u8()?;
        p.ib = r.get_u8()?;
        let c = &mut self.ctl;
        c.command = r.get_u8()?;
        c.read_buffer.copy_from_slice(r.get_bytes(8)?);
        c.write_buffer.copy_from_slice(r.get_bytes(8)?);
        c.read_pos = r.get_u8()?;
        c.write_pos = r.get_u8()?;
        c.last_port_b = r.get_u8()?;
        c.inter_vbl_cycles = r.get_u16()?;
        c.operating_mode = r.get_u8()?;
        c.int_state = r.get_u8()?;
        c.cur_x = r.get_u16()? as i16;
        c.cur_y = r.get_u16()? as i16;
        c.last_x = r.get_u16()? as i16;
        c.last_y = r.get_u16()? as i16;
        c.min_x = r.get_u16()? as i16;
        c.min_y = r.get_u16()? as i16;
        c.max_x = r.get_u16()? as i16;
        c.max_y = r.get_u16()? as i16;
        let b = r.get_u8()?;
        c.cur_b0 = b & 1 != 0;
        c.cur_b1 = b & 2 != 0;
        c.last_b0 = b & 4 != 0;
        c.last_b1 = b & 8 != 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed ROM is the real `342-0270-C` image, pinned by SHA-1 (like
    /// the Apple ][ ROM set) and by its identification bytes.
    #[test]
    fn mouse_rom_is_the_committed_image() {
        assert_eq!(MOUSE_ROM.len(), 2048);
        let sha1: String = crate::ws::sha1(MOUSE_ROM)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(sha1, "3a9d881a8a8d30f55b9719aceebbcf717f829d6f");
        // Identification bytes read from the default bank (page 0): the Pascal
        // 1.1 protocol signature, the X-Y pointing-device class, the mouse ID,
        // and $Cn01 != $20 so the slot scan never mistakes it for a Disk II.
        assert_eq!(MOUSE_ROM[0x05], 0x38);
        assert_eq!(MOUSE_ROM[0x07], 0x18);
        assert_eq!(MOUSE_ROM[0x0c], 0x20);
        assert_eq!(MOUSE_ROM[0xfb], 0xd6);
        assert_ne!(MOUSE_ROM[0x01], 0x20);
    }

    /// The card serves the identification bytes from the default bank, and PIA
    /// port B bits 1-3 switch which of the eight ROM pages is visible at
    /// `$Cn00`.
    #[test]
    fn port_b_banks_the_slot_rom() {
        let mut m = Mou::new(4);
        // At reset port B is 0 → page 0, which carries the ID bytes.
        assert_eq!(m.read(0xc405, 0), 0x38, "$Cn05 from the default bank");
        assert_eq!(m.read(0xc4fb, 0), 0xd6, "$CnFB from the default bank");

        // Select each page through port B bits 1-3 and confirm the window
        // tracks it. (DDRB = 0x3E puts bits 1-5 under 6502 control.)
        pia_init(&mut m);
        for bank in 0u8..8 {
            set_port_b(&mut m, (bank << 1) & 0x0e);
            for off in [0x00u8, 0x05, 0x70, 0xff] {
                assert_eq!(
                    m.read(0xc400 + off as u16, 0),
                    MOUSE_ROM[bank as usize * 256 + off as usize],
                    "bank {bank} offset ${off:02x}"
                );
            }
        }
    }

    /// The PIA's data-vs-direction select (`CRx bit 2`) and the physical port
    /// value `(OR & DDR) | (IN & ~DDR)`.
    #[test]
    fn pia_data_direction_select() {
        let mut m = Mou::new(4);
        // Write DDRB with CRB bit 2 clear; read it back the same way.
        m.write(0xc0c3, 0x00, 0); // CRB: DDR access
        m.write(0xc0c2, 0x3e, 0); // DDRB = 0x3E
        assert_eq!(m.read(0xc0c2, 0), 0x3e, "reads DDRB when CRB bit2=0");
        // Data access: the ROM-bank output bits (1-3) reflect ORB where DDRB=1.
        // (The handshake input bits 6-7 are driven by the controller, so mask
        // to the bank field.)
        m.write(0xc0c3, 0x04, 0); // CRB: data access
        m.write(0xc0c2, 0x0c, 0); // ORB bank bits, no handshake bits
        assert_eq!(m.read(0xc0c2, 0) & 0x0e, 0x0c, "bank bits reflect ORB");
        // The control register keeps only bits 0-5.
        m.write(0xc0c3, 0xff, 0);
        assert_eq!(m.read(0xc0c3, 0), 0x3f);
    }

    // ---- a simulated 6502 side of the handshake (what the ROM firmware does),
    // so the controller can be exercised without executing the ROM (that is the
    // firmware-driven gate in P2). ----

    const DEVSEL: u16 = 0xc0c0; // slot-4 base; the low two bits decode the PIA

    fn wr(m: &mut Mou, reg: u16, v: u8) {
        m.write(DEVSEL + reg, v, 0);
    }
    fn rd(m: &mut Mou, reg: u16) -> u8 {
        m.read(DEVSEL + reg, 0)
    }

    /// Initialise the PIA as the ROM does: port B DDR = 0x3E, both ports in
    /// data-access mode.
    fn pia_init(m: &mut Mou) {
        wr(m, 3, 0x00); // CRB: DDR access
        wr(m, 2, 0x3e); // DDRB = 0x3E
        wr(m, 3, 0x04); // CRB: data access
        wr(m, 2, 0x00); // ORB = 0
        wr(m, 1, 0x04); // CRA: data access
    }

    /// Set the port B output bits (ROM page + handshake), preserving nothing —
    /// the caller supplies the full ORB value.
    fn set_port_b(m: &mut Mou, orb: u8) {
        wr(m, 2, orb);
    }

    fn set_ddra(m: &mut Mou, ddra: u8) {
        wr(m, 1, 0x00); // CRA: DDR access
        wr(m, 0, ddra);
        wr(m, 1, 0x04); // CRA: data access
    }

    /// Poll a settled condition; the state is fixed after each write, so a true
    /// condition is seen at once and a stuck one (a handshake bug) panics
    /// rather than hanging like the real timeout-free firmware.
    fn poll(m: &mut Mou, mut cond: impl FnMut(&mut Mou) -> bool, what: &str) {
        for _ in 0..1000 {
            if cond(m) {
                return;
            }
        }
        panic!("handshake stuck waiting for {what}");
    }

    /// The write half: send one byte 6502→6805.
    fn send_byte(m: &mut Mou, byte: u8) {
        set_ddra(m, 0xff); // port A output
        wr(m, 0, byte); // ORA = byte
        set_port_b(m, PB_WRREQUEST);
        poll(m, |m| rd(m, 2) & PB_WRACK != 0, "WRACK");
        set_port_b(m, 0x00);
        poll(m, |m| rd(m, 2) & PB_WRACK == 0, "not WRACK");
    }

    /// The read half: receive one byte 6805→6502.
    fn recv_byte(m: &mut Mou) -> u8 {
        set_ddra(m, 0x00); // port A input
        poll(m, |m| rd(m, 2) & PB_RDREADY != 0, "RDREADY");
        let byte = rd(m, 0);
        set_port_b(m, PB_RDACK);
        poll(m, |m| rd(m, 2) & PB_RDREADY == 0, "not RDREADY");
        set_port_b(m, 0x00);
        byte
    }

    fn command(m: &mut Mou, cmd: u8, params: &[u8]) {
        send_byte(m, cmd);
        for &p in params {
            send_byte(m, p);
        }
    }

    /// ClampMouse streams min-lo, max-lo, min-hi, max-hi (`command_clamp`'s
    /// interleave); axis 0 = X, 1 = Y.
    fn clamp(m: &mut Mou, axis: u8, min: i16, max: i16) {
        command(
            m,
            CMD_CLAMPMOUSE | axis,
            &[min as u8, max as u8, (min >> 8) as u8, (max >> 8) as u8],
        );
    }

    /// PosMouse streams x-lo, x-hi, y-lo, y-hi.
    fn pos(m: &mut Mou, x: i16, y: i16) {
        command(
            m,
            CMD_POSMOUSE,
            &[x as u8, (x >> 8) as u8, y as u8, (y >> 8) as u8],
        );
    }

    /// ReadMouse replies with X-lo, X-hi, Y-lo, Y-hi, status.
    fn read_mouse(m: &mut Mou) -> (u16, u16, u8) {
        send_byte(m, CMD_READMOUSE);
        let xl = recv_byte(m);
        let xh = recv_byte(m);
        let yl = recv_byte(m);
        let yh = recv_byte(m);
        let status = recv_byte(m);
        (
            u16::from_le_bytes([xl, xh]),
            u16::from_le_bytes([yl, yh]),
            status,
        )
    }

    /// The P1 handshake gate: Init → Clamp → Pos → Read through the real PIA
    /// handshake and command engine deposits the clamped position and status.
    #[test]
    fn firmware_handshake_init_clamp_pos_read() {
        let mut m = Mou::new(4);
        pia_init(&mut m);
        command(&mut m, CMD_INITMOUSE, &[]);
        clamp(&mut m, 0, 100, 700); // X in 100..=700
        clamp(&mut m, 1, 200, 500); // Y in 200..=500
        command(&mut m, CMD_SETMOUSE | 0x01, &[]); // mode = mouse on
        pos(&mut m, 400, 400);
        let (x, y, _status) = read_mouse(&mut m);
        assert_eq!((x, y), (400, 400), "inside the clamp window");

        // Outside the window: pinned at the bounds, not wrapped.
        pos(&mut m, 9999, -9999);
        let (x, y, _) = read_mouse(&mut m);
        assert_eq!((x, y), (700, 200), "clamped to (maxX, minY)");
    }

    /// The status byte tracks the button (now / at last read) and movement
    /// since the last read, across the handshake.
    #[test]
    fn status_tracks_button_and_movement() {
        let mut m = Mou::new(4);
        pia_init(&mut m);
        command(&mut m, CMD_INITMOUSE, &[]);
        let (_, _, status) = read_mouse(&mut m);
        assert_eq!(status, 0, "fresh: no button, no movement");

        m.set_button(true);
        m.set_position(300, 300);
        let (_, _, status) = read_mouse(&mut m);
        assert_eq!(status & STATUS_IS_BUTTON0, STATUS_IS_BUTTON0, "button now");
        assert_eq!(status & STATUS_MOVED, STATUS_MOVED, "moved since last read");

        // Hold, don't move: button now + at last read, moved cleared.
        let (_, _, status) = read_mouse(&mut m);
        assert_eq!(
            status & (STATUS_IS_BUTTON0 | STATUS_WAS_BUTTON0),
            STATUS_IS_BUTTON0 | STATUS_WAS_BUTTON0
        );
        assert_eq!(status & STATUS_MOVED, 0, "no movement since the last read");
    }

    /// HomeMouse parks at the clamp minimum.
    #[test]
    fn home_moves_to_clamp_minimum() {
        let mut m = Mou::new(4);
        pia_init(&mut m);
        clamp(&mut m, 0, 100, 200);
        clamp(&mut m, 1, 300, 400);
        command(&mut m, CMD_HOMEMOUSE, &[]);
        let (x, y, _) = read_mouse(&mut m);
        assert_eq!((x, y), (100, 300));
    }

    /// The GETCLAMP workaround (RDMEMMOUSE) reads back the clamp boundaries.
    #[test]
    fn rdmem_reads_the_clamp_boundaries() {
        let mut m = Mou::new(4);
        pia_init(&mut m);
        clamp(&mut m, 0, 100, 700); // X: min 100 ($0064), max 700 ($02BC)
        send_byte(&mut m, CMD_RDMEMMOUSE);
        send_byte(&mut m, 0x49); // address low ($0049 = MinXL)
        send_byte(&mut m, 0x00); // address high
        assert_eq!(recv_byte(&mut m), 0x64, "MinX low byte");
    }

    /// Interrupt plumbing: VBL / movement / button raise the line only when
    /// enabled; ServeMouse reports the source and de-asserts.
    #[test]
    fn interrupts_are_mode_gated_and_serve_clears() {
        let mut m = Mou::new(4);
        m.vbl_tick();
        assert!(!m.irq_asserted(), "no interrupt without the enable bits");

        m.set_operating_mode(MODE_VBL_IRQ); // VBL interrupt
        m.vbl_tick();
        assert!(m.irq_asserted(), "VBL raises the line when enabled");
        m.run_command(CMD_SERVEMOUSE);
        assert!(!m.irq_asserted(), "ServeMouse cleared it");

        m.set_operating_mode(MODE_MOVED_IRQ); // mouse on + movement interrupt
        m.set_position(400, 400);
        assert!(m.irq_asserted(), "movement raises the line");
        m.run_command(CMD_SERVEMOUSE);
        assert!(!m.irq_asserted());

        m.set_operating_mode(MODE_BUTTON_IRQ); // mouse on + button interrupt
        m.set_button(true);
        assert!(m.irq_asserted(), "button raises the line");
    }

    /// Full card state round-trips (notes/STATE.md).
    #[test]
    fn state_round_trips() {
        use ewm_core::state::Persist;
        let mut m = Mou::new(4);
        pia_init(&mut m);
        clamp(&mut m, 0, 50, 900);
        pos(&mut m, 123, 456);
        m.set_button(true);
        let mut w = ewm_core::state::Writer::new();
        m.save(&mut w);
        let bytes = w.into_bytes();

        let mut m2 = Mou::new(4);
        let mut r = ewm_core::state::Reader::new(&bytes);
        m2.restore(&mut r).unwrap();
        assert_eq!(m2.position(), (123, 456));
        assert_eq!(m2.clamp(), (50, 900, 0, 1023));
        assert_eq!(m2.status_byte() & STATUS_IS_BUTTON0, STATUS_IS_BUTTON0);
    }
}
