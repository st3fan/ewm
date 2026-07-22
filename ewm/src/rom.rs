//! The embedded ROM catalog: one table keyed by SKU (or a product slug for
//! ROMs with no Apple part number) → `(description, bytes)`, the single source
//! of every ROM baked into the binary. Consumers — `two.rs` (machine ROMs),
//! `chr.rs` (character/video ROMs), `mouse.rs` (the AppleMouse card ROM), and
//! the `builtin:` config resolver — fetch bytes by key with `rom()` / hold the
//! `&'static [u8]` from construction. The `desc` field carries the "what is
//! this ROM" story that used to live in scattered `//` comments.
//!
//! See notes/ROMS.md (the design) and plans/20260720-03-rom-catalog.md.

/// One embedded ROM.
pub struct RomEntry {
    /// Catalog key: the Apple part number (SKU, e.g. `342-0304-A`), or a
    /// product slug for ROMs with no part number (e.g. `WozMon`).
    pub key: &'static str,
    /// One-line description of what the ROM is.
    pub desc: &'static str,
    /// The ROM bytes.
    pub data: &'static [u8],
}

/// The embedded ROM catalog. Keys are unique (enforced by
/// `catalog_keys_are_unique`), kept sorted by key. The `builtin:` config
/// resolver (R3) exposes the SKU-less entries — and any SKU — by key.
pub static ROM_CATALOG: &[RomEntry] = &[
    // --- Apple ][ (1978): Integer BASIC, Programmer's Aid, Original Monitor ---
    RomEntry {
        key: "341-0001",
        desc: "Apple ][ Integer BASIC, $E000-$E7FF",
        data: include_bytes!("../../roms/341-0001 — Apple II Integer BASIC E000 (2716).bin"),
    },
    RomEntry {
        key: "341-0002",
        desc: "Apple ][ Integer BASIC, $E800-$EFFF",
        data: include_bytes!("../../roms/341-0002 — Apple II Integer BASIC E800 (2716).bin"),
    },
    RomEntry {
        key: "341-0003",
        desc: "Apple ][ Integer BASIC, $F000-$F7FF",
        data: include_bytes!("../../roms/341-0003 — Apple II Integer BASIC F000 (2716).bin"),
    },
    RomEntry {
        key: "341-0004",
        desc: "Apple ][ Original (non-autostart) Monitor, $F800-$FFFF",
        data: include_bytes!("../../roms/341-0004 — Apple II Original Monitor F800 (2716).bin"),
    },
    // --- Apple ][+ (1979): AppleSoft BASIC + Autostart Monitor ---
    RomEntry {
        key: "341-0011",
        desc: "Apple ][+ AppleSoft BASIC, $D000-$D7FF",
        data: include_bytes!("../../roms/341-0011 — Apple II+ AppleSoft BASIC D000 (2716).bin"),
    },
    RomEntry {
        key: "341-0012",
        desc: "Apple ][+ AppleSoft BASIC, $D800-$DFFF",
        data: include_bytes!("../../roms/341-0012 — Apple II+ AppleSoft BASIC D800 (2716).bin"),
    },
    RomEntry {
        key: "341-0013",
        desc: "Apple ][+ AppleSoft BASIC, $E000-$E7FF",
        data: include_bytes!("../../roms/341-0013 — Apple II+ AppleSoft BASIC E000 (2716).bin"),
    },
    RomEntry {
        key: "341-0014",
        desc: "Apple ][+ AppleSoft BASIC, $E800-$EFFF",
        data: include_bytes!("../../roms/341-0014 — Apple II+ AppleSoft BASIC E800 (2716).bin"),
    },
    RomEntry {
        key: "341-0015",
        desc: "Apple ][+ AppleSoft BASIC, $F000-$F7FF",
        data: include_bytes!("../../roms/341-0015 — Apple II+ AppleSoft BASIC F000 (2716).bin"),
    },
    RomEntry {
        key: "341-0016",
        desc: "Apple ][ Programmer's Aid #1, $D000-$D7FF",
        data: include_bytes!("../../roms/341-0016 — Apple II Programmer's Aid #1 D000 (2716).bin"),
    },
    RomEntry {
        key: "341-0020",
        desc: "Apple ][+ Autostart Monitor, $F800-$FFFF",
        data: include_bytes!("../../roms/341-0020 — Apple II+ Autostart Monitor F800 (2716).bin"),
    },
    // --- Character / video ROMs ---
    RomEntry {
        key: "341-0036",
        desc: "Apple ][ character ROM",
        data: include_bytes!("../../roms/341-0036 — Apple II Character ROM (2513).bin"),
    },
    RomEntry {
        key: "342-0133-A",
        desc: "Original //e video ROM (no MouseText)",
        data: include_bytes!("../../roms/342-0133-A — Apple IIe Video Unenhanced (2732).bin"),
    },
    RomEntry {
        key: "342-0265-A",
        desc: "Enhanced //e video ROM (with MouseText)",
        data: include_bytes!("../../roms/342-0265-A — Apple IIe Video Enhanced (2732).bin"),
    },
    // --- //e system ROM halves (CD = $C000-$DFFF, EF = $E000-$FFFF) ---
    RomEntry {
        key: "342-0134-A",
        desc: "Original //e system ROM, EF half ($E000-$FFFF)",
        data: include_bytes!("../../roms/342-0134-A — Apple IIe EF Unenhanced (2764).bin"),
    },
    RomEntry {
        key: "342-0135-B",
        desc: "Original //e system ROM, CD half ($C000-$DFFF)",
        data: include_bytes!("../../roms/342-0135-B — Apple IIe CD Unenhanced (2764).bin"),
    },
    RomEntry {
        key: "342-0303-A",
        desc: "Enhanced //e system ROM, EF half ($E000-$FFFF)",
        data: include_bytes!("../../roms/342-0303-A — Apple IIe EF Enhanced (2764).bin"),
    },
    RomEntry {
        key: "342-0304-A",
        desc: "Enhanced //e system ROM, CD half ($C000-$DFFF)",
        data: include_bytes!("../../roms/342-0304-A — Apple IIe CD Enhanced (2764).bin"),
    },
    // --- Peripheral card ROMs ---
    RomEntry {
        key: "342-0270-C",
        desc: "AppleMouse II interface card ROM (banked into $Cn00)",
        data: include_bytes!("../../roms/342-0270-C — AppleMouse II Interface Card (2716).bin"),
    },
    // --- ROMs with no Apple part number (keyed by product slug); these are
    // the ones the built-in Apple 1 / Replica 1 configs mount by `builtin:`. ---
    RomEntry {
        key: "Krusader-1.3-6502",
        desc: "Krusader 1.3 assembler/monitor (6502)",
        data: include_bytes!("../../roms/Krusader-1.3-6502.bin"),
    },
    RomEntry {
        key: "Krusader-1.3-65C02",
        desc: "Krusader 1.3 assembler/monitor (65C02)",
        data: include_bytes!("../../roms/Krusader-1.3-65C02.bin"),
    },
    RomEntry {
        key: "WozMon",
        desc: "Apple 1 Woz Monitor ($FF00-$FFFF)",
        data: include_bytes!("../../roms/WozMon.bin"),
    },
    RomEntry {
        key: "apple1-basic",
        desc: "Apple 1 Integer BASIC ($E000-$EFFF)",
        data: include_bytes!("../../roms/apple1-basic.bin"),
    },
];

/// The bytes of the ROM with catalog key `key`, or `None` if there is no such
/// entry.
pub fn catalog(key: &str) -> Option<&'static [u8]> {
    ROM_CATALOG.iter().find(|e| e.key == key).map(|e| e.data)
}

/// The bytes of the ROM with catalog key `key`, panicking on an unknown key —
/// for the compile-time-known keys the machine builders and devices use.
pub fn rom(key: &str) -> &'static [u8] {
    catalog(key).unwrap_or_else(|| panic!("ROM catalog has no key {key:?}"))
}

/// The one-line description of the ROM with catalog key `key` — the text the
/// config files put in a `//` comment behind each `builtin:<key>`.
pub fn describe(key: &str) -> Option<&'static str> {
    ROM_CATALOG.iter().find(|e| e.key == key).map(|e| e.desc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every catalog key is unique, every entry has a description and a
    /// plausible ROM-sized, non-empty payload (a 256 B page up to a 64 KB
    /// image, a whole number of 256-byte pages).
    #[test]
    fn catalog_is_well_formed() {
        let mut seen = HashSet::new();
        for e in ROM_CATALOG {
            assert!(seen.insert(e.key), "duplicate catalog key {:?}", e.key);
            assert!(!e.desc.is_empty(), "{:?} has no description", e.key);
            assert!(!e.data.is_empty(), "{:?} has no bytes", e.key);
            assert!(
                e.data.len() % 256 == 0 && e.data.len() <= 0x10000,
                "{:?} has an implausible length {}",
                e.key,
                e.data.len()
            );
        }
    }

    #[test]
    fn lookup_resolves_and_rejects() {
        assert_eq!(rom("341-0036").len(), 2048);
        assert_eq!(rom("342-0304-A").len(), 8192);
        assert!(catalog("342-0270-C").is_some());
        assert!(catalog("no-such-rom").is_none());
    }
}
