//! The Apple //e auxiliary slot: a trait for the memory card in it, with one
//! implementation file per card. `IouE` resolves *which* kind of aux access
//! an address is — the `$0000-$BFFF` body via RAMRD/RAMWRT/ALTZP, the
//! 80STORE/video display pages, or the aux language card — and the card
//! decides what memory, if any, answers. Cards decode their own aux-slot
//! registers from `$C070-$C07F` writes (RamWorks: the `$C073` bank select).
//!
//! Unpopulated reads float `0xFF`; unpopulated writes are dropped — sizing
//! probes rely on absent memory not aliasing bank 0.

mod ext80;
mod ramworks;
mod text80;

pub use ext80::Ext80Col;
pub use ramworks::RamWorksIII;
pub use text80::Text80Col;

/// The aux language-card regions the //e MMU resolves before asking the
/// card: the two `$D000` banks and the `$E000-$FFFF` high region.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LcRegion {
    Bank1,
    Bank2,
    High,
}

/// A card in the //e auxiliary slot.
pub trait AuxCard {
    /// RAMRD/RAMWRT/ALTZP access to the selected bank's `$0000-$BFFF`.
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, b: u8);

    /// The 80STORE display pages and the video scanner: bank 0 on RamWorks,
    /// the same 64K on the extended card, the 1K text page on the plain
    /// 80-column card.
    fn video_read(&self, addr: u16) -> u8;
    fn video_write(&mut self, addr: u16, b: u8);

    /// The renderer's aux view: a `$0000-$BFFF`-shaped slice (regions a card
    /// does not populate read as zero).
    fn video_ram(&self) -> &[u8];

    /// The aux language card of the selected bank.
    fn lc_read(&self, region: LcRegion, offset: usize) -> u8;
    fn lc_write(&mut self, region: LcRegion, offset: usize, b: u8);

    /// Aux-slot-visible I/O writes (the `$C070-$C07F` range is forwarded);
    /// cards decode their own registers. Default: ignore.
    fn io_write(&mut self, _addr: u16, _b: u8) {}

    /// Human label, e.g. "RamWorks III (8 MB)".
    fn label(&self) -> String;
}

/// The size of one auxiliary bank's `$0000-$BFFF` body.
pub(crate) const BODY_SIZE: usize = 0xC000;

/// One complete 64K auxiliary bank, shared by the extended card and
/// RamWorks: the 48K body plus the aux language card
/// (48K + 4K + 4K + 8K = exactly 64K).
pub struct AuxBank {
    pub ram: Vec<u8>,   // $0000-$BFFF
    pub lc_d1: Vec<u8>, // $D000 RAM bank 1
    pub lc_d2: Vec<u8>, // $D000 RAM bank 2
    pub lc_e: Vec<u8>,  // $E000-$FFFF
}

impl AuxBank {
    pub fn new() -> AuxBank {
        AuxBank {
            ram: vec![0; BODY_SIZE],
            lc_d1: vec![0; 0x1000],
            lc_d2: vec![0; 0x1000],
            lc_e: vec![0; 0x2000],
        }
    }

    pub fn lc(&self, region: LcRegion) -> &[u8] {
        match region {
            LcRegion::Bank1 => &self.lc_d1,
            LcRegion::Bank2 => &self.lc_d2,
            LcRegion::High => &self.lc_e,
        }
    }

    pub fn lc_mut(&mut self, region: LcRegion) -> &mut [u8] {
        match region {
            LcRegion::Bank1 => &mut self.lc_d1,
            LcRegion::Bank2 => &mut self.lc_d2,
            LcRegion::High => &mut self.lc_e,
        }
    }
}

impl Default for AuxBank {
    fn default() -> AuxBank {
        AuxBank::new()
    }
}

/// Build the card for a `--aux` flag value: `80col` (the 1K Apple 80-Column
/// Text Card), `ext80col`/`std` (the Extended 80-Column Text Card, 64K —
/// the default card), or `ramworksiii[:SIZE]` with SIZE any multiple of 64K
/// from `64k` to `8m` (omitted: `8m`).
pub fn parse(s: &str) -> Result<Box<dyn AuxCard>, String> {
    match s {
        "80col" => Ok(Box::new(Text80Col::new())),
        "ext80col" | "std" => Ok(Box::new(Ext80Col::new())),
        "ramworksiii" => Ok(Box::new(RamWorksIII::new(128))),
        _ => {
            if let Some(size) = s.strip_prefix("ramworksiii:") {
                let bytes = parse_size(size)?;
                Ok(Box::new(RamWorksIII::new(bytes / 0x10000)))
            } else {
                Err(format!(
                    "unknown aux card {s:?} (expected 80col, ext80col or ramworksiii[:SIZE])"
                ))
            }
        }
    }
}

/// Parse a memory size like `256k`, `1m` or `8m` into bytes; must be a
/// multiple of 64K between 64K and 8M (the RamWorks III maximum).
fn parse_size(s: &str) -> Result<usize, String> {
    let lower = s.to_lowercase();
    let (digits, unit) = lower.split_at(lower.len().saturating_sub(1));
    let multiplier = match unit {
        "k" => 1024,
        "m" => 1024 * 1024,
        _ => return Err(format!("bad size {s:?} (expected e.g. 256k or 1m)")),
    };
    let n: usize = digits
        .parse()
        .map_err(|_| format!("bad size {s:?} (expected e.g. 256k or 1m)"))?;
    let bytes = n * multiplier;
    if bytes == 0 || !bytes.is_multiple_of(0x10000) || bytes > 8 * 1024 * 1024 {
        return Err(format!(
            "bad size {s:?}: must be a multiple of 64k between 64k and 8m"
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_builds_the_right_cards() {
        assert_eq!(parse("80col").unwrap().label(), "80-Column Text Card (1K)");
        for name in ["ext80col", "std"] {
            assert_eq!(
                parse(name).unwrap().label(),
                "Extended 80-Column Text Card (64K)"
            );
        }
        assert_eq!(parse("ramworksiii").unwrap().label(), "RamWorks III (8 MB)");
        assert_eq!(
            parse("ramworksiii:256k").unwrap().label(),
            "RamWorks III (256 KB)"
        );
        assert_eq!(
            parse("ramworksiii:1m").unwrap().label(),
            "RamWorks III (1 MB)"
        );
    }

    #[test]
    fn parse_rejects_nonsense() {
        assert!(parse("ramworks").is_err());
        assert!(parse("ramworksiii:").is_err());
        assert!(parse("ramworksiii:0k").is_err());
        assert!(parse("ramworksiii:100k").is_err()); // not a 64K multiple
        assert!(parse("ramworksiii:16m").is_err()); // beyond the card's max
        assert!(parse("ramworksiii:64g").is_err());
        assert!(parse("vidhd").is_err());
    }
}
