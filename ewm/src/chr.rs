//! Character ROM decoding, port of the bitmap half of `chr.c`: the Apple ][
//! character ROM (`341-0036`) becomes per-character 7×8 glyph bitmaps,
//! indexed by Apple ][ screen code. Texture creation from these bitmaps is
//! frontend work (Phase 7); nothing here touches SDL.

pub const CHR_WIDTH: usize = 7;
pub const CHR_HEIGHT: usize = 8;

// The character / video ROMs come from the catalog (`rom::rom`): the ][ 2K
// character ROM (341-0036), and the two 4K //e video ROMs — Enhanced
// (342-0265-A, with MouseText at $40-$5F) and original (342-0133-A, no
// MouseText; inverse upper case there instead). Only the first 2K of each //e
// ROM is used for display; inverse forms are synthesized by XOR, so each
// ROM's baked-inverse second half is unused.

pub type Glyph = [bool; CHR_WIDTH * CHR_HEIGHT];

pub struct Chr {
    bitmaps: [Option<Glyph>; 256],
}

/// Port of `_generate_bitmap`: eight rows starting at `rom[c * 8 + 1]`
/// (note the one-byte offset), seven pixels per row scanned from bit 6
/// down to bit 0.
fn generate_bitmap(rom: &[u8], c: usize, inverse: bool) -> Glyph {
    let mut glyph = [false; CHR_WIDTH * CHR_HEIGHT];
    let mut p = 0;
    for y in 0..CHR_HEIGHT {
        let mut row = rom[(c * 8) + y + 1];
        if inverse {
            row ^= 0xff;
        }
        for x in (0..CHR_WIDTH).rev() {
            glyph[p] = row & (1 << x) != 0;
            p += 1;
        }
    }
    glyph
}

impl Chr {
    /// Port of `ewm_chr_init`'s bitmap tables: normal text at `$A0-$DF`,
    /// inverse at `$00-$3F`, flashing (currently rendered as inverse, as in
    /// C) at `$40-$7F`. Slots `$80-$9F` and `$E0-$FF` stay empty and render
    /// blank — the Apple 1 character set has no lower case.
    // The index loops deliberately mirror ewm_chr_init's six table-fill
    // loops rather than being rewritten in iterator style.
    #[allow(clippy::needless_range_loop)]
    pub fn new() -> Chr {
        let rom = crate::rom::rom("341-0036");
        let mut bitmaps: [Option<Glyph>; 256] = [None; 256];

        // Normal Text
        for c in 0..32 {
            bitmaps[0xc0 + c] = Some(generate_bitmap(rom, c, false));
        }
        for c in 32..64 {
            bitmaps[0xa0 + (c - 32)] = Some(generate_bitmap(rom, c, false));
        }

        // Inverse Text
        for c in 0..32 {
            bitmaps[c] = Some(generate_bitmap(rom, c, true));
        }
        for c in 32..64 {
            bitmaps[0x20 + (c - 32)] = Some(generate_bitmap(rom, c, true));
        }

        // TODO Flashing - Currently simply rendered as inverse
        for c in 0..32 {
            bitmaps[0x40 + c] = Some(generate_bitmap(rom, c, true));
        }
        for c in 32..64 {
            bitmaps[0x60 + (c - 32)] = Some(generate_bitmap(rom, c, true));
        }

        Chr { bitmaps }
    }

    /// The glyph for an Apple ][ screen code, or `None` for unmapped codes.
    pub fn bitmap(&self, screen_code: u8) -> Option<&Glyph> {
        self.bitmaps[screen_code as usize].as_ref()
    }
}

impl Default for Chr {
    fn default() -> Chr {
        Chr::new()
    }
}

/// The two Apple //e character sets, selected at display time by the
/// ALTCHARSET soft switch (`$C00E`/`$C00F`, reported by `$C01E`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CharSet {
    /// Upper case, symbols and **lower case** (the normal `$80-$FF` range,
    /// including lower case at `$E0-$FF`), with `$00-$3F` inverse and `$40-$7F`
    /// flashing. No MouseText and no inverse lower case — that is the only
    /// difference from the alternate set.
    Primary,
    /// Adds MouseText and inverse lower case in place of the primary set's
    /// flashing `$40-$7F`: inverse UC/sym (`$00-$3F`), MouseText (`$40-$5F`),
    /// inverse lower case (`$60-$7F`), normal UC/sym/lower case (`$80-$FF`).
    Alternate,
}

/// Decode one glyph from the enhanced //e 4K video ROM: eight rows starting
/// at `rom[idx * 8]` (no `+1` offset, unlike the ][+ 2716 dump). Unlike the
/// ][+ ROM, the //e ROM stores the leftmost pixel in **bit 0**, so bits are
/// scanned low-to-high (bit 0 → bit 6); reading them high-to-low mirrors the
/// glyph horizontally.
fn generate_bitmap_iie(rom: &[u8], idx: usize, inverse: bool) -> Glyph {
    let mut glyph = [false; CHR_WIDTH * CHR_HEIGHT];
    let mut p = 0;
    for y in 0..CHR_HEIGHT {
        let mut row = rom[(idx * 8) + y];
        if inverse {
            row ^= 0xff;
        }
        for x in 0..CHR_WIDTH {
            glyph[p] = row & (1 << x) != 0;
            p += 1;
        }
    }
    glyph
}

/// Primary-set screen code → (ROM glyph index, inverse?). The primary and
/// alternate sets are identical for the normal ranges — including **lower case
/// at `$E0-$FF`** — and differ only in `$40-$7F`, which the primary set flashes
/// (upper case / symbols) where the alternate set shows MouseText and inverse
/// lower case. Apple's own firmware relies on this: it prints "Apple //e" with
/// lower-case codes while the primary set is selected (`$C00E`). Flashing codes
/// (`$40-$7F`) are rendered in their inverse phase, as in the ][+ decode above.
fn primary_index(code: u8) -> (usize, bool) {
    match code {
        0xe0..=0xff => (((code & 0x1f) | 0x60) as usize, false), // normal lower case
        _ => ((code & 0x3f) as usize, code < 0x80),
    }
}

/// Alternate-set screen code → (ROM glyph index, inverse?). This is the //e
/// video display-code translation, derived from the ROM layout: lower case is
/// read from ROM `$60-$7F`. The `$40-$5F` slot is the one place the two //e
/// generations differ: the Enhanced set reads MouseText straight from ROM
/// `$40-$5F` (`mousetext`), while the original //e shows **inverse upper
/// case** there — the same inverse glyphs the primary set flashes, just
/// steady (Apple replaced them with MouseText when it enhanced the //e).
fn alternate_index(code: u8, mousetext: bool) -> (usize, bool) {
    match code {
        0x00..=0x3f => ((code & 0x3f) as usize, true), // inverse UC / symbols
        0x40..=0x5f if mousetext => (code as usize, false), // Enhanced: MouseText (as stored)
        0x40..=0x5f => ((code & 0x3f) as usize, true), // original //e: inverse UC / symbols
        0x60..=0x7f => (code as usize, true),          // inverse lower case
        0x80..=0xdf => ((code & 0x3f) as usize, false), // normal UC / symbols
        0xe0..=0xff => (((code & 0x1f) | 0x60) as usize, false), // normal lower case
    }
}

/// The Enhanced //e glyph tables: both character sets fully decoded, indexed
/// by screen code. Every code maps to a glyph (unlike the ][+ `Chr`, which
/// leaves lower-case and unused codes blank), so lookups return `&Glyph`.
pub struct ChrE {
    sets: [[Glyph; 256]; 2],
}

impl ChrE {
    /// The Enhanced //e glyph tables (342-0265 video ROM, with MouseText).
    pub fn new() -> ChrE {
        ChrE::from_rom(crate::rom::rom("342-0265-A"), true)
    }

    /// The original (unenhanced) //e glyph tables (342-0133 video ROM, no
    /// MouseText). The decode is the same //e layout; only the alternate
    /// `$40-$5F` slot differs (inverse upper case, not MouseText — see
    /// `alternate_index`).
    pub fn new_unenhanced() -> ChrE {
        ChrE::from_rom(crate::rom::rom("342-0133-A"), false)
    }

    fn from_rom(rom: &[u8], mousetext: bool) -> ChrE {
        let mut sets = [[[false; CHR_WIDTH * CHR_HEIGHT]; 256]; 2];
        for (code, glyph) in sets[CharSet::Primary as usize].iter_mut().enumerate() {
            let (idx, inverse) = primary_index(code as u8);
            *glyph = generate_bitmap_iie(rom, idx, inverse);
        }
        for (code, glyph) in sets[CharSet::Alternate as usize].iter_mut().enumerate() {
            let (idx, inverse) = alternate_index(code as u8, mousetext);
            *glyph = generate_bitmap_iie(rom, idx, inverse);
        }
        ChrE { sets }
    }

    /// The glyph for a screen code in the given character set.
    pub fn bitmap(&self, set: CharSet, screen_code: u8) -> &Glyph {
        &self.sets[set as usize][screen_code as usize]
    }

    /// The glyph for a screen code, selecting the set from the ALTCHARSET soft
    /// switch (`$C01E`): alternate (lower case + MouseText) when on, primary
    /// (upper case + symbols) when off.
    pub fn glyph(&self, altcharset: bool, screen_code: u8) -> &Glyph {
        let set = if altcharset {
            CharSet::Alternate
        } else {
            CharSet::Primary
        };
        self.bitmap(set, screen_code)
    }
}

impl Default for ChrE {
    fn default() -> ChrE {
        ChrE::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Provenance for the unenhanced //e video ROM (E2 of
    /// plans/20260720-02): pinned by SHA-1 so the original-//e character set
    /// E3 renders from it cannot silently drift.
    #[test]
    fn iie_unenhanced_video_rom_matches_the_committed_image() {
        let rom = crate::rom::rom("342-0133-A");
        let hex: String = crate::ws::sha1(rom)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(rom.len(), 4096);
        assert_eq!(hex, "58ad0008df72896a18601e090ee0d58155ffa5be");
    }

    /// E3: the original //e has no MouseText. Its alternate character set
    /// shows **inverse upper case** at `$40-$5F` — the same glyphs the
    /// primary set flashes there, steady — which is exactly what Apple
    /// replaced with MouseText when it enhanced the //e. So those glyphs
    /// differ from the Enhanced //e's MouseText, and the two sets are
    /// otherwise byte-identical.
    #[test]
    fn iie_original_alternate_set_is_inverse_upper_case_not_mousetext() {
        let enhanced = ChrE::new();
        let original = ChrE::new_unenhanced();

        for code in 0x40u8..=0x5f {
            // Original: the steady alternate glyph is the primary set's
            // inverse-upper-case glyph for the same code.
            assert_eq!(
                original.bitmap(CharSet::Alternate, code),
                original.bitmap(CharSet::Primary, code),
                "original //e alt ${code:02X} is the steady inverse-UC glyph"
            );
            // Enhanced: MouseText there — a different glyph.
            assert_ne!(
                enhanced.bitmap(CharSet::Alternate, code),
                original.bitmap(CharSet::Alternate, code),
                "Enhanced MouseText must differ from the original at ${code:02X}"
            );
        }

        // Everything else is byte-identical between the two //e sets: the
        // primary set entirely, and the alternate set outside $40-$5F.
        for code in 0u16..=0xff {
            let c = code as u8;
            assert_eq!(
                enhanced.bitmap(CharSet::Primary, c),
                original.bitmap(CharSet::Primary, c),
                "primary ${c:02X} identical"
            );
            if !(0x40..=0x5f).contains(&c) {
                assert_eq!(
                    enhanced.bitmap(CharSet::Alternate, c),
                    original.bitmap(CharSet::Alternate, c),
                    "alternate ${c:02X} identical"
                );
            }
        }
    }

    fn render(glyph: &Glyph) -> String {
        glyph
            .chunks(CHR_WIDTH)
            .map(|row| {
                row.iter()
                    .map(|&on| if on { 'X' } else { '.' })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn glyph_a_decodes_to_expected_bitmap() {
        // 'A' is screen code $C1 (normal text).
        let chr = Chr::new();
        let glyph = chr.bitmap(0xc1).expect("no glyph for $C1");
        let expected = "\
...X...
..X.X..
.X...X.
.X...X.
.XXXXX.
.X...X.
.X...X.
.......";
        assert_eq!(render(glyph), expected);
    }

    #[test]
    fn inverse_glyph_is_inverted() {
        let chr = Chr::new();
        let normal = chr.bitmap(0xc1).unwrap(); // 'A' normal
        let inverse = chr.bitmap(0x01).unwrap(); // 'A' inverse
        for (n, i) in normal.iter().zip(inverse.iter()) {
            assert_eq!(*n, !*i);
        }
    }

    #[test]
    fn unmapped_codes_have_no_glyph() {
        let chr = Chr::new();
        assert!(chr.bitmap(0x80).is_none()); // $80-$9F unmapped
        assert!(chr.bitmap(0xe1).is_none()); // lower case 'a' unmapped
        assert!(chr.bitmap(0xa0).is_some()); // space, mapped
    }

    // --- Enhanced //e character sets ---

    #[test]
    fn iie_primary_uppercase_a() {
        // Normal 'A' is screen code $C1 in both sets.
        let chr = ChrE::new();
        let expected = "\
...X...
..X.X..
.X...X.
.X...X.
.XXXXX.
.X...X.
.X...X.
.......";
        assert_eq!(render(chr.bitmap(CharSet::Primary, 0xc1)), expected);
    }

    #[test]
    fn iie_primary_inverse_is_xor_of_normal() {
        // $01 is inverse 'A', $C1 is normal 'A'.
        let chr = ChrE::new();
        let normal = chr.bitmap(CharSet::Primary, 0xc1);
        let inverse = chr.bitmap(CharSet::Primary, 0x01);
        for (n, i) in normal.iter().zip(inverse.iter()) {
            assert_eq!(*n, !*i);
        }
    }

    #[test]
    fn iie_primary_has_lower_case() {
        // Both //e sets show lower case in the normal $E0-$FF range: the
        // primary set's $E1 is the same lower-case 'a' as the alternate set's,
        // and is NOT the symbol '!' ($A1). (The sets differ only in $40-$7F.)
        // Apple's firmware depends on this — it prints "Apple //e" with
        // lower-case codes while the primary set is selected.
        let chr = ChrE::new();
        assert_eq!(
            chr.bitmap(CharSet::Primary, 0xe1),
            chr.bitmap(CharSet::Alternate, 0xe1),
            "primary $E1 is lower-case 'a', as in the alternate set"
        );
        assert_ne!(
            chr.bitmap(CharSet::Primary, 0xe1),
            chr.bitmap(CharSet::Primary, 0xa1),
            "lower-case 'a' ($E1) is not the symbol '!' ($A1)"
        );
    }

    #[test]
    fn iie_primary_and_alternate_match_across_normal_range() {
        // The two sets are identical for every normal code $80-$FF (upper case,
        // symbols and lower case). They diverge only in $40-$7F (flash vs
        // MouseText / inverse lower case).
        let chr = ChrE::new();
        for code in 0x80u8..=0xff {
            assert_eq!(
                chr.bitmap(CharSet::Primary, code),
                chr.bitmap(CharSet::Alternate, code),
                "primary and alternate differ at normal code ${code:02X}"
            );
        }
    }

    #[test]
    fn iie_alternate_lower_case_a() {
        // Lower-case 'a' is screen code $E1 in the alternate set (and the
        // primary set — both //e sets render lower case; the ][+ cannot).
        let chr = ChrE::new();
        let expected = "\
.......
.......
..XXX..
.....X.
..XXXX.
.X...X.
..XXXX.
.......";
        assert_eq!(render(chr.bitmap(CharSet::Alternate, 0xe1)), expected);
    }

    #[test]
    fn iie_alternate_inverse_lower_case() {
        // $61 is inverse lower-case 'a', $E1 is normal lower-case 'a'.
        let chr = ChrE::new();
        let normal = chr.bitmap(CharSet::Alternate, 0xe1);
        let inverse = chr.bitmap(CharSet::Alternate, 0x61);
        for (n, i) in normal.iter().zip(inverse.iter()) {
            assert_eq!(*n, !*i);
        }
    }

    #[test]
    fn iie_alternate_uppercase_matches_primary() {
        // Both sets share the same letter glyphs; 'A' ($C1) is identical.
        let chr = ChrE::new();
        assert_eq!(
            chr.bitmap(CharSet::Alternate, 0xc1),
            chr.bitmap(CharSet::Primary, 0xc1)
        );
    }

    #[test]
    fn iie_alternate_mousetext_checkerboard() {
        // MouseText occupies alternate codes $40-$5F. $56 is the checkerboard
        // dither glyph — a clean, unambiguous target.
        let chr = ChrE::new();
        let expected = "\
X.X.X.X
.X.X.X.
X.X.X.X
.X.X.X.
X.X.X.X
.X.X.X.
X.X.X.X
.X.X.X.";
        assert_eq!(render(chr.bitmap(CharSet::Alternate, 0x56)), expected);
    }

    #[test]
    fn iie_primary_shows_no_mousetext() {
        // The primary set never shows MouseText: $56 maps to an upper-case /
        // symbol glyph ($56 & $3F = $16 = 'V'), not the checkerboard.
        let chr = ChrE::new();
        assert_ne!(
            chr.bitmap(CharSet::Primary, 0x56),
            chr.bitmap(CharSet::Alternate, 0x56)
        );
    }
}
