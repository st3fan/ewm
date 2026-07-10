//! The Applied Engineering RamWorks III: up to 128 × 64K auxiliary banks.
//! A write to `$C073` selects the bank that replaces auxiliary memory in
//! all the //e addressing mechanisms (RAMRD/RAMWRT, ALTZP, the aux language
//! card) — **except** the 80STORE display pages and the video scanner,
//! which always refer to bank 0 (the 80-column video buffers only exist
//! there). The register is 8-bit and write-only; selecting an unpopulated
//! bank floats (reads `0xFF`, writes dropped), which is what sizing probes
//! depend on.

use super::{AuxBank, AuxCard, LcRegion};

pub struct RamWorksIII {
    banks: Vec<AuxBank>,
    /// The raw `$C073` value; may point past the populated banks.
    selected: u8,
}

impl RamWorksIII {
    /// A card with `banks` × 64K (1..=128; 128 = the full 8 MB).
    pub fn new(banks: usize) -> RamWorksIII {
        RamWorksIII {
            banks: (0..banks.clamp(1, 128)).map(|_| AuxBank::new()).collect(),
            selected: 0,
        }
    }

    fn bank(&self) -> Option<&AuxBank> {
        self.banks.get(self.selected as usize)
    }

    fn bank_mut(&mut self) -> Option<&mut AuxBank> {
        self.banks.get_mut(self.selected as usize)
    }
}

impl AuxCard for RamWorksIII {
    fn read(&self, addr: u16) -> u8 {
        self.bank().map_or(0xff, |bank| bank.ram[addr as usize])
    }

    fn write(&mut self, addr: u16, b: u8) {
        if let Some(bank) = self.bank_mut() {
            bank.ram[addr as usize] = b;
        }
    }

    fn video_read(&self, addr: u16) -> u8 {
        self.banks[0].ram[addr as usize]
    }

    fn video_write(&mut self, addr: u16, b: u8) {
        self.banks[0].ram[addr as usize] = b;
    }

    fn video_ram(&self) -> &[u8] {
        &self.banks[0].ram
    }

    fn lc_read(&self, region: LcRegion, offset: usize) -> u8 {
        self.bank().map_or(0xff, |bank| bank.lc(region)[offset])
    }

    fn lc_write(&mut self, region: LcRegion, offset: usize, b: u8) {
        if let Some(bank) = self.bank_mut() {
            bank.lc_mut(region)[offset] = b;
        }
    }

    fn io_write(&mut self, addr: u16, b: u8) {
        if addr == 0xc073 {
            self.selected = b;
        }
    }

    fn label(&self) -> String {
        let kb = self.banks.len() * 64;
        if kb.is_multiple_of(1024) {
            format!("RamWorks III ({} MB)", kb / 1024)
        } else {
            format!("RamWorks III ({kb} KB)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banks_are_selected_at_c073_and_isolated() {
        let mut card = RamWorksIII::new(4); // 256 KB
        card.write(0x0300, 0x10);
        card.io_write(0xc073, 1);
        card.write(0x0300, 0x11);
        assert_eq!(card.read(0x0300), 0x11);
        card.io_write(0xc073, 0);
        assert_eq!(card.read(0x0300), 0x10, "bank 0 kept its own value");

        // The language card is per-bank too.
        card.lc_write(LcRegion::Bank1, 0, 0xa0);
        card.io_write(0xc073, 2);
        assert_eq!(card.lc_read(LcRegion::Bank1, 0), 0x00);
    }

    #[test]
    fn video_is_pinned_to_bank_0() {
        let mut card = RamWorksIII::new(4);
        card.io_write(0xc073, 3);
        card.video_write(0x0400, 0x42);
        assert_eq!(card.read(0x0400), 0x00, "CPU sees bank 3, untouched");
        card.io_write(0xc073, 0);
        assert_eq!(card.read(0x0400), 0x42, "the write landed in bank 0");
        assert_eq!(card.video_ram()[0x0400], 0x42);
    }

    #[test]
    fn unpopulated_banks_float() {
        let mut card = RamWorksIII::new(4);
        card.write(0x0300, 0x10);
        card.io_write(0xc073, 9); // past the 4 populated banks
        assert_eq!(card.read(0x0300), 0xff, "reads float");
        card.write(0x0300, 0x99); // dropped
        assert_eq!(card.lc_read(LcRegion::High, 0), 0xff);
        card.io_write(0xc073, 0);
        assert_eq!(card.read(0x0300), 0x10, "populated data intact");
    }

    #[test]
    fn labels_follow_capacity() {
        assert_eq!(RamWorksIII::new(4).label(), "RamWorks III (256 KB)");
        assert_eq!(RamWorksIII::new(16).label(), "RamWorks III (1 MB)");
        assert_eq!(RamWorksIII::new(128).label(), "RamWorks III (8 MB)");
    }
}
