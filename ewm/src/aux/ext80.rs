//! The Apple Extended 80-Column Text Card: one full 64K auxiliary bank —
//! body, display pages and aux language card all answer from the same
//! memory. This is the default aux card and exactly the behavior the //e
//! had before the aux slot became swappable.

use super::{AuxBank, AuxCard, LcRegion};

pub struct Ext80Col {
    bank: AuxBank,
}

impl Ext80Col {
    pub fn new() -> Ext80Col {
        Ext80Col {
            bank: AuxBank::new(),
        }
    }
}

impl Default for Ext80Col {
    fn default() -> Ext80Col {
        Ext80Col::new()
    }
}

impl AuxCard for Ext80Col {
    fn read(&self, addr: u16) -> u8 {
        self.bank.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, b: u8) {
        self.bank.ram[addr as usize] = b;
    }

    fn video_read(&self, addr: u16) -> u8 {
        self.read(addr)
    }

    fn video_write(&mut self, addr: u16, b: u8) {
        self.write(addr, b);
    }

    fn video_ram(&self) -> &[u8] {
        &self.bank.ram
    }

    fn lc_read(&self, region: LcRegion, offset: usize) -> u8 {
        self.bank.lc(region)[offset]
    }

    fn lc_write(&mut self, region: LcRegion, offset: usize, b: u8) {
        self.bank.lc_mut(region)[offset] = b;
    }

    fn label(&self) -> String {
        "Extended 80-Column Text Card (64K)".to_string()
    }
}

/// The full 64K auxiliary bank (notes/STATE.md §5).
impl ewm_core::state::Persist for Ext80Col {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        self.bank.save_state(w);
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.bank.restore_state(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_video_and_lc_share_the_single_bank() {
        let mut card = Ext80Col::new();
        card.write(0x0400, 0x11);
        assert_eq!(card.video_read(0x0400), 0x11, "video sees body writes");
        card.video_write(0x2000, 0x22);
        assert_eq!(card.read(0x2000), 0x22, "body sees video writes");
        assert_eq!(card.video_ram()[0x2000], 0x22);

        card.lc_write(LcRegion::Bank1, 0, 0x33);
        assert_eq!(card.lc_read(LcRegion::Bank1, 0), 0x33);
        assert_eq!(card.lc_read(LcRegion::Bank2, 0), 0x00, "banks distinct");

        // The bank-select register of other cards is ignored here.
        card.io_write(0xc073, 5);
        assert_eq!(card.read(0x0400), 0x11, "no banking on the extended card");
    }
}
