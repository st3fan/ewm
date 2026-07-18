//! The plain Apple 80-Column Text Card: 1K of memory that answers only at
//! the auxiliary half of text page 1 (`$0400-$07FF`). 80-column text works;
//! RAMRD/RAMWRT, ALTZP, the aux language card and double-resolution
//! graphics have no memory behind them (reads float `0xFF`, writes drop).

use super::{AuxCard, BODY_SIZE, LcRegion};

pub struct Text80Col {
    /// A `$0000-$BFFF`-shaped backing so the renderer can index it like any
    /// aux bank; only `$0400-$07FF` is ever written (the card's 1K).
    ram: Vec<u8>,
}

/// The only address range the card decodes.
const PAGE: std::ops::Range<u16> = 0x0400..0x0800;

impl Text80Col {
    pub fn new() -> Text80Col {
        Text80Col {
            ram: vec![0; BODY_SIZE],
        }
    }
}

impl Default for Text80Col {
    fn default() -> Text80Col {
        Text80Col::new()
    }
}

impl AuxCard for Text80Col {
    fn read(&self, addr: u16) -> u8 {
        if PAGE.contains(&addr) {
            self.ram[addr as usize]
        } else {
            0xff
        }
    }

    fn write(&mut self, addr: u16, b: u8) {
        if PAGE.contains(&addr) {
            self.ram[addr as usize] = b;
        }
    }

    fn video_read(&self, addr: u16) -> u8 {
        self.read(addr)
    }

    fn video_write(&mut self, addr: u16, b: u8) {
        self.write(addr, b);
    }

    fn video_ram(&self) -> &[u8] {
        &self.ram
    }

    fn lc_read(&self, _region: LcRegion, _offset: usize) -> u8 {
        0xff // no aux language card on the 1K card
    }

    fn lc_write(&mut self, _region: LcRegion, _offset: usize, _b: u8) {}

    fn label(&self) -> String {
        "80-Column Text Card (1K)".to_string()
    }
}

/// The card's 1K, saved as its full `$0000-$BFFF`-shaped backing
/// (notes/STATE.md §5).
impl ewm_core::state::Persist for Text80Col {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_blob(&self.ram);
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        crate::alc::restore_ram(&mut self.ram, r, "80-column card RAM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_text_page_answers() {
        let mut card = Text80Col::new();
        card.write(0x0400, 0x11);
        card.write(0x07ff, 0x22);
        assert_eq!(card.read(0x0400), 0x11);
        assert_eq!(card.read(0x07ff), 0x22);
        assert_eq!(card.video_read(0x0400), 0x11, "video shares the 1K");

        // Everything outside $0400-$07FF floats / drops.
        card.write(0x0300, 0x33);
        card.write(0x2000, 0x44);
        assert_eq!(card.read(0x0300), 0xff);
        assert_eq!(card.read(0x2000), 0xff);
        assert_eq!(card.video_ram()[0x2000], 0x00, "renderer sees zeros");

        // No aux language card.
        card.lc_write(LcRegion::High, 0, 0x55);
        assert_eq!(card.lc_read(LcRegion::High, 0), 0xff);

        // No bank register either.
        card.io_write(0xc073, 3);
        assert_eq!(card.read(0x0400), 0x11);
    }
}
