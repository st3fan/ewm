//! 6820 Peripheral I/O Adapter, port of `pia.c`. On the Apple 1 this is
//! what connects the keyboard and display logic to the CPU. The
//! implementation is not complete but does enough to support how the
//! keyboard and display are hooked up. As in C the PIA registers itself as
//! an IO region; the output callback becomes a drainable queue.

use ewm_core::mem::Device;

pub const PIA6820_DDRA: u8 = 0;
pub const PIA6820_CTLA: u8 = 1;
pub const PIA6820_DDRB: u8 = 2;
pub const PIA6820_CTLB: u8 = 3;

pub const A1_PIA6820_ADDR: u16 = 0xd010;
pub const A1_PIA6820_LENGTH: u16 = 0x0100;

const KBD_DDR: u16 = A1_PIA6820_ADDR + PIA6820_DDRA as u16;
const KBD_CTL: u16 = A1_PIA6820_ADDR + PIA6820_CTLA as u16;
const DSP_DDR: u16 = A1_PIA6820_ADDR + PIA6820_DDRB as u16;
const DSP_CTL: u16 = A1_PIA6820_ADDR + PIA6820_CTLB as u16;

#[derive(Default)]
pub struct Pia {
    pub ina: u8,
    pub outa: u8,
    pub ddra: u8,
    pub ctla: u8,
    pub inb: u8,
    pub outb: u8,
    pub ddrb: u8,
    pub ctlb: u8,
    out: Vec<(u8, u8)>,
}

impl Pia {
    pub fn new() -> Pia {
        Pia::default()
    }

    pub fn set_ina(&mut self, v: u8) {
        self.ina = v;
    }

    pub fn set_irqa1(&mut self) {
        self.ctla |= 0b1000_0000; // Set IRQA1
    }

    /// `(ddr, v)` pairs written to the output registers since the last
    /// drain — the C output callback turned into a queue for the machine to
    /// route to its display sink.
    pub fn drain_out(&mut self) -> Vec<(u8, u8)> {
        std::mem::take(&mut self.out)
    }

    /// The keyboard latch still holds an unread key (IRQA1 set; reading
    /// `$D010` clears it) — the pacing predicate for senders that must
    /// not overwrite the one-byte latch (the tty frontend).
    pub fn key_pending(&self) -> bool {
        self.ctla & 0b1000_0000 != 0
    }
}

impl Device for Pia {
    /// Port of `pia_read`. Reading the keyboard register clears IRQA1.
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        match addr {
            KBD_DDR => {
                if self.ctla & 0b0000_0100 != 0 {
                    self.ctla &= 0b0111_1111; // Clear IRQA1
                    (self.outa & self.ddra) | (self.ina & !self.ddra)
                } else {
                    self.ddra
                }
            }
            KBD_CTL => self.ctla,
            DSP_DDR => {
                if self.ctlb & 0b0000_0100 != 0 {
                    (self.outb & self.ddrb) | (self.inb & !self.ddrb)
                } else {
                    self.ddrb
                }
            }
            DSP_CTL => self.ctlb,
            _ => 0,
        }
    }

    /// Port of `pia_write`. Output-register writes are queued for
    /// `drain_out`, where the C invoked the callback.
    fn write(&mut self, addr: u16, v: u8, _cycles: u64) {
        match addr {
            KBD_DDR => {
                // Check B2 (DDR Access)
                if self.ctla & 0b0000_0100 != 0 {
                    self.outa = v;
                    self.out.push((PIA6820_DDRA, v));
                } else {
                    self.ddra = v;
                }
            }
            KBD_CTL => self.ctla = v & 0b0011_1111,
            DSP_DDR => {
                // Check B2 (DDR Access)
                if self.ctlb & 0b0000_0100 != 0 {
                    self.outb = v;
                    self.out.push((PIA6820_DDRB, v));
                } else {
                    self.ddrb = v;
                }
            }
            DSP_CTL => self.ctlb = v & 0b0011_1111,
            _ => {}
        }
    }
}

/// PIA registers and the undrained output queue (notes/STATE.md §5) — the
/// queue is normally empty at the frame boundary, but saving it costs
/// nothing and loses nothing.
impl ewm_core::state::Persist for Pia {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_u8(self.ina);
        w.put_u8(self.outa);
        w.put_u8(self.ddra);
        w.put_u8(self.ctla);
        w.put_u8(self.inb);
        w.put_u8(self.outb);
        w.put_u8(self.ddrb);
        w.put_u8(self.ctlb);
        w.put_u16(self.out.len() as u16);
        for &(ddr, v) in &self.out {
            w.put_u8(ddr);
            w.put_u8(v);
        }
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.ina = r.get_u8()?;
        self.outa = r.get_u8()?;
        self.ddra = r.get_u8()?;
        self.ctla = r.get_u8()?;
        self.inb = r.get_u8()?;
        self.outb = r.get_u8()?;
        self.ddrb = r.get_u8()?;
        self.ctlb = r.get_u8()?;
        let count = r.get_u16()? as usize;
        self.out.clear();
        for _ in 0..count {
            let ddr = r.get_u8()?;
            let v = r.get_u8()?;
            self.out.push((ddr, v));
        }
        Ok(())
    }
}
