//! Saturn Systems 128K RAM Board — the classic third-party slot 0 card
//! (Saturn Systems, Inc., Ann Arbor; Operations Manual ch. 9). 128K as
//! eight 16K banks that all live at `$D000-$FFFF`: each 16K bank is two
//! 4K banks (A/B) selectable at `$D000-$DFFF` plus its own 8K at
//! `$E000-$FFFF`. Bank 1 speaks the Apple Language Card protocol exactly,
//! which is the card's compatibility story — DOS 3.3, Pascal and VisiCalc
//! treat it as a 16K card and never notice the other seven banks.
//!
//! Control is the 16 locations `$C080-$C08F` (slot 0), decoded on any
//! access:
//!
//! - **A2=0 — state selection** (offsets 0-3, 8-B): the Language Card
//!   protocol. A3 selects the 4K bank at `$D000` (0=A, 1=B); A0/A1 pick
//!   RAM/ROM read and write enable, same table as `crate::alc`.
//! - **A2=1 — 16K bank selection** (offsets 4-7, C-F):
//!   bank = `((off & 8) >> 1) | (off & 3)` — `$C084-7` are banks 1-4,
//!   `$C08C-F` banks 5-8. Read/write/4K-bank state persists across bank
//!   switches (one board-level state, per the manual's LED table).
//!
//! Write-enable mirrors `Alc`: two consecutive *reads* of a write-enable
//! offset are required, and any `$C08x` write resets the count without
//! ever enabling. The manual's prose is looser ("a POKE or PEEK will
//! accomplish this"), but the LC-compatible read-twice protocol is what
//! real software uses and what the Language Card itself implements.
//!
//! Power-up state: ROM read, write protect, bank 1 — the board
//! effectively disabled.

use ewm_core::mem::Device;

pub struct Saturn {
    /// The selected 16K bank, 0-7 (the manual's "16K Bank 1"-"8").
    bank: usize,
    /// A3 of the last state-selection access: true is the B 4K bank at
    /// `$D000`.
    bank_b: bool,
    read: bool,
    write: bool,
    wrtcount: u32,
    ram: Vec<u8>, // 128K, bank-contiguous: 4K A + 4K B + 8K per 16K bank
    rom: Vec<u8>, // 12K machine ROM at $D000, the fall-through target
}

impl Saturn {
    /// `rom` is the combined `$D000-$FFFF` machine ROM the card shadows.
    pub fn new(rom: Vec<u8>) -> Saturn {
        assert_eq!(rom.len(), 0x3000, "machine ROM must cover $D000-$FFFF");
        Saturn {
            bank: 0,
            bank_b: false,
            read: false,
            write: false,
            wrtcount: 0,
            ram: vec![0; 8 * 0x4000],
            rom,
        }
    }

    /// The selected 16K bank, 0-7 (for WozBug's SLOTS display).
    pub fn bank(&self) -> usize {
        self.bank
    }

    /// The `$C08x` decode shared by reads and writes: A2 high is 16K bank
    /// selection, A2 low latches A3 as the 4K bank. Returns whether the
    /// access was a bank selection (state bits untouched).
    fn select(&mut self, addr: u16) -> bool {
        if addr & 0b0100 != 0 {
            self.bank = (((addr & 0b1000) >> 1) | (addr & 0b0011)) as usize;
            return true;
        }
        self.bank_b = addr & 0b1000 != 0;
        false
    }

    /// `$C080-$C08F` read: the `Alc::iom_read` state machine plus bank
    /// selection. Always returns 0.
    fn iom_read(&mut self, addr: u16) -> u8 {
        if self.select(addr) {
            return 0;
        }
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

    /// `$C080-$C08F` write: writes always reset the write count and never
    /// enable writes, as on the Language Card.
    fn iom_write(&mut self, addr: u16) {
        if self.select(addr) {
            return;
        }
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

    /// The RAM index for a `$D000-$FFFF` address in the selected bank.
    fn index(&self, addr: u16) -> usize {
        self.bank * 0x4000
            + match addr {
                0xd000..=0xdfff => (self.bank_b as usize) * 0x1000 + (addr - 0xd000) as usize,
                _ => 0x2000 + (addr - 0xe000) as usize,
            }
    }

    /// `$D000-$FFFF` read: card RAM when read-enabled, else the machine
    /// ROM.
    fn bank_read(&self, addr: u16) -> u8 {
        if self.read {
            self.ram[self.index(addr)]
        } else {
            self.rom[(addr - 0xd000) as usize]
        }
    }

    /// `$D000-$FFFF` write, swallowed while write-protected (ROM swallows
    /// the rest).
    fn bank_write(&mut self, addr: u16, b: u8) {
        if self.write {
            let index = self.index(addr);
            self.ram[index] = b;
        }
    }
}

impl Device for Saturn {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        match addr {
            0xc080..=0xc08f => self.iom_read(addr),
            _ => self.bank_read(addr),
        }
    }

    fn write(&mut self, addr: u16, b: u8, _cycles: u64) {
        match addr {
            0xc080..=0xc08f => self.iom_write(addr),
            _ => self.bank_write(addr, b),
        }
    }
}

/// Banking state and the 128K (notes/STATE.md §5); the shadowed machine ROM
/// is construction data and not written.
impl ewm_core::state::Persist for Saturn {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_u8(self.bank as u8);
        w.put_bool(self.bank_b);
        w.put_bool(self.read);
        w.put_bool(self.write);
        w.put_u32(self.wrtcount);
        w.put_blob(&self.ram);
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.bank = r.get_u8()? as usize % 8;
        self.bank_b = r.get_bool()?;
        self.read = r.get_bool()?;
        self.write = r.get_bool()?;
        self.wrtcount = r.get_u32()?;
        crate::alc::restore_ram(&mut self.ram, r, "Saturn RAM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saturn() -> Saturn {
        // A recognizable ROM: every byte is the low byte of its address.
        let rom: Vec<u8> = (0..0x3000u32).map(|i| i as u8).collect();
        Saturn::new(rom)
    }

    /// Read-enable RAM and write-enable it (two reads of $C083).
    fn arm(s: &mut Saturn) {
        s.read(0xc083, 0);
        s.read(0xc083, 0);
    }

    #[test]
    fn powers_up_reading_rom_write_protected() {
        let mut s = saturn();
        assert_eq!(s.bank(), 0);
        assert_eq!(s.read(0xd000, 0), 0x00, "ROM byte at $D000");
        assert_eq!(s.read(0xe000, 0), 0x00, "ROM byte at $E000");
        assert_eq!(s.read(0xffff, 0), 0xff, "ROM byte at $FFFF");
        s.write(0xd000, 0x42, 0);
        arm(&mut s);
        assert_eq!(s.read(0xd000, 0), 0x00, "the write must have bounced");
    }

    #[test]
    fn state_offsets_decode_like_the_language_card() {
        let mut s = saturn();
        // $C083/$C083: RAM read, write enable.
        arm(&mut s);
        assert!(s.read && s.write);
        // $C080: RAM read, write protect.
        s.read(0xc080, 0);
        assert!(s.read && !s.write);
        // $C081/$C081: ROM read, write enable.
        s.read(0xc081, 0);
        s.read(0xc081, 0);
        assert!(!s.read && s.write);
        // $C082: ROM read, write protect — the power-up state.
        s.read(0xc082, 0);
        assert!(!s.read && !s.write);
        // The 8-B column repeats the table with the B 4K bank.
        s.read(0xc08b, 0);
        s.read(0xc08b, 0);
        assert!(s.read && s.write && s.bank_b);
        s.read(0xc083, 0);
        assert!(!s.bank_b, "A3 low returns to the A bank");
    }

    #[test]
    fn write_enable_needs_two_reads_and_writes_reset_the_count() {
        let mut s = saturn();
        s.read(0xc083, 0);
        assert!(!s.write, "one read is not enough");
        s.write(0xc083, 0, 0);
        s.read(0xc083, 0);
        assert!(!s.write, "a $C08x write must reset the count");
        s.read(0xc083, 0);
        assert!(s.write, "two consecutive reads write-enable");
        s.write(0xc083, 0, 0);
        assert!(s.write, "a write leaves an enabled write alone");
    }

    #[test]
    fn bank_select_offsets_map_per_the_manual() {
        // $C084-7 are 16K banks 1-4, $C08C-F banks 5-8 (0-7 here).
        let mut s = saturn();
        for (offset, bank) in [
            (0xc084, 0),
            (0xc085, 1),
            (0xc086, 2),
            (0xc087, 3),
            (0xc08c, 4),
            (0xc08d, 5),
            (0xc08e, 6),
            (0xc08f, 7),
        ] {
            s.read(offset, 0);
            assert_eq!(s.bank(), bank, "offset {offset:04X}");
        }
    }

    #[test]
    fn banks_hold_independent_contents_and_state_persists() {
        let mut s = saturn();
        arm(&mut s);
        for bank in [0usize, 1, 7] {
            let offset = if bank < 4 {
                0xc084 + bank
            } else {
                0xc088 + bank
            } as u16;
            s.write(offset, 0, 0); // bank select by write, too
            s.write(0xd000, 0x10 + bank as u8, 0);
            s.write(0xe000, 0x20 + bank as u8, 0);
            s.write(0xffff, 0x30 + bank as u8, 0);
            assert!(s.write, "bank switching must not disturb write enable");
        }
        for bank in [0usize, 1, 7] {
            let offset = if bank < 4 {
                0xc084 + bank
            } else {
                0xc088 + bank
            } as u16;
            s.read(offset, 0);
            assert_eq!(s.read(0xd000, 0), 0x10 + bank as u8);
            assert_eq!(s.read(0xe000, 0), 0x20 + bank as u8);
            assert_eq!(s.read(0xffff, 0), 0x30 + bank as u8);
        }
    }

    #[test]
    fn the_two_4k_banks_split_d000_and_share_nothing() {
        let mut s = saturn();
        arm(&mut s); // $C083: RAM read, write enable, 4K bank A
        s.write(0xd000, 0xaa, 0);
        s.write(0xe000, 0xee, 0);
        s.read(0xc08b, 0); // 4K bank B, same read/write state
        assert_eq!(s.read(0xd000, 0), 0x00, "bank B starts empty");
        s.write(0xd000, 0xbb, 0);
        assert_eq!(s.read(0xe000, 0), 0xee, "$E000-$FFFF is shared A/B");
        s.read(0xc083, 0); // back to A
        assert_eq!(s.read(0xd000, 0), 0xaa);
    }

    #[test]
    fn rom_falls_through_when_ram_reads_are_disabled() {
        let mut s = saturn();
        arm(&mut s);
        s.write(0xd123, 0x5a, 0);
        s.read(0xc081, 0); // ROM read
        assert_eq!(s.read(0xd123, 0), 0x23, "ROM byte, not the RAM value");
        s.read(0xc080, 0); // RAM read again
        assert_eq!(s.read(0xd123, 0), 0x5a, "the RAM value survived");
    }
}
