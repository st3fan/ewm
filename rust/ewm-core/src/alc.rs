//! Apple Language Card, port of `alc.c`. The C implementation toggles
//! enabled/flags bits on three `mem_t` regions; here the same state lives in
//! plain fields that `Two`'s bus dispatch consults for `$D000-$FFFF`.
//!
//! Semantics preserved from the C handlers:
//! - Any `$C08x` access selects the banks: bit 3 set → RAM1 at `$D000`,
//!   clear → RAM2; RAM3 at `$E000` is always enabled after the first access.
//! - *Reads* of `$C081/$C085/$C089/$C08D` (and the `3` column) bump the
//!   write count and enable writes at 2 — and leave a previously enabled
//!   write alone. *Writes* to any `$C08x` reset the count, so write-enable
//!   requires two consecutive reads.

pub struct Alc {
    /// False until the first `$C08x` access — all card RAM disabled, exactly
    /// like the three regions starting out `enabled = false` in C.
    active: bool,
    /// Bit 3 of the last `$C08x` access: true selects RAM1 for `$D000`.
    bank1: bool,
    read: bool,
    write: bool,
    wrtcount: u32,
    ram1: Vec<u8>, // 4K at $D000
    ram2: Vec<u8>, // 4K at $D000
    ram3: Vec<u8>, // 8K at $E000
}

impl Alc {
    pub fn new() -> Alc {
        Alc {
            active: false,
            bank1: false,
            read: false,
            write: false,
            wrtcount: 0,
            ram1: vec![0; 4096],
            ram2: vec![0; 4096],
            ram3: vec![0; 8192],
        }
    }

    fn select_banks(&mut self, addr: u16) {
        self.active = true;
        self.bank1 = addr & 0b0000_1000 != 0;
    }

    /// Port of `alc_iom_read` for `$C080-$C08F`. Always returns 0.
    pub fn iom_read(&mut self, addr: u16) -> u8 {
        self.select_banks(addr);
        match addr & 0b0011 {
            // WRTCOUNT = 0, WRITE DISABLE, READ ENABLE
            0b00 => {
                self.wrtcount = 0;
                self.read = true;
                self.write = false;
            }
            // WRTCOUNT++, READ DISABLE, WRITE ENABLE IF WRTCOUNT >= 2
            0b01 => {
                self.wrtcount += 1;
                self.read = false;
                if self.wrtcount >= 2 {
                    self.write = true;
                }
            }
            // WRTCOUNT = 0, WRITE DISABLE, READ DISABLE
            0b10 => {
                self.wrtcount = 0;
                self.read = false;
                self.write = false;
            }
            // WRTCOUNT++, READ ENABLE, WRITE ENABLE IF WRTCOUNT >= 2
            _ => {
                self.wrtcount += 1;
                self.read = true;
                if self.wrtcount >= 2 {
                    self.write = true;
                }
            }
        }
        0
    }

    /// Port of `alc_iom_write` for `$C080-$C08F`. Writes always reset the
    /// write count and never enable writes.
    pub fn iom_write(&mut self, addr: u16) {
        self.select_banks(addr);
        match addr & 0b0011 {
            // WRTCOUNT = 0, WRITE DISABLE, READ ENABLE
            0b00 => {
                self.wrtcount = 0;
                self.read = true;
                self.write = false;
            }
            // WRTCOUNT = 0, READ DISABLE
            0b01 => {
                self.wrtcount = 0;
                self.read = false;
            }
            // WRTCOUNT = 0, WRITE DISABLE, READ DISABLE
            0b10 => {
                self.wrtcount = 0;
                self.read = false;
                self.write = false;
            }
            // WRTCOUNT = 0, READ ENABLE
            _ => {
                self.wrtcount = 0;
                self.read = true;
            }
        }
    }

    /// Card RAM read for `$D000-$FFFF`; `None` falls through to ROM, like a
    /// disabled or read-disabled region falling through the C mem list.
    pub fn read(&self, addr: u16) -> Option<u8> {
        if !self.active || !self.read {
            return None;
        }
        match addr {
            0xd000..=0xdfff => {
                let bank = if self.bank1 { &self.ram1 } else { &self.ram2 };
                Some(bank[(addr - 0xd000) as usize])
            }
            0xe000..=0xffff => Some(self.ram3[(addr - 0xe000) as usize]),
            _ => None,
        }
    }

    /// Card RAM write for `$D000-$FFFF`. When the card is inactive or
    /// write-disabled the write is swallowed, matching the C mem walk where
    /// a matched region without the write flag returns without writing (and
    /// ROM swallows the rest).
    pub fn write(&mut self, addr: u16, b: u8) {
        if !self.active || !self.write {
            return;
        }
        match addr {
            0xd000..=0xdfff => {
                let bank = if self.bank1 {
                    &mut self.ram1
                } else {
                    &mut self.ram2
                };
                bank[(addr - 0xd000) as usize] = b;
            }
            0xe000..=0xffff => self.ram3[(addr - 0xe000) as usize] = b,
            _ => {}
        }
    }
}

impl Default for Alc {
    fn default() -> Alc {
        Alc::new()
    }
}
