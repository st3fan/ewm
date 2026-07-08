//! Character ROM decoding, port of the bitmap half of `chr.c`: the 2716
//! character ROM (`3410036.bin`) becomes per-character 7×8 glyph bitmaps,
//! indexed by Apple ][ screen code. Texture creation from these bitmaps is
//! frontend work (Phase 7); nothing here touches SDL.

pub const CHR_WIDTH: usize = 7;
pub const CHR_HEIGHT: usize = 8;

static CHR_ROM: &[u8; 2048] = include_bytes!("../../rom/3410036.bin");

/// The Enhanced Apple //e 4K video ROM (`342-0265-A`). Only the first 2K is
/// used for display: it holds the complete glyph repertoire — upper case and
/// symbols (`$00-$3F`), MouseText (`$40-$5F`), and lower case (`$60-$7F`) —
/// from which both //e character sets are built. Inverse forms are
/// synthesized by XOR, as for the ][+ set, so the ROM's baked-inverse second
/// half is not needed.
static CHR_ROM_IIE: &[u8; 4096] =
    include_bytes!("../../rom/Apple IIe Video - Enhanced - 342-0265-A - 2732.bin");

type Glyph = [bool; CHR_WIDTH * CHR_HEIGHT];

pub struct Chr {
    bitmaps: [Option<Glyph>; 256],
}

/// Port of `_generate_bitmap`: eight rows starting at `rom[c * 8 + 1]`
/// (note the one-byte offset), seven pixels per row scanned from bit 6
/// down to bit 0.
fn generate_bitmap(c: usize, inverse: bool) -> Glyph {
    let mut glyph = [false; CHR_WIDTH * CHR_HEIGHT];
    let mut p = 0;
    for y in 0..CHR_HEIGHT {
        let mut row = CHR_ROM[(c * 8) + y + 1];
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
        let mut bitmaps: [Option<Glyph>; 256] = [None; 256];

        // Normal Text
        for c in 0..32 {
            bitmaps[0xc0 + c] = Some(generate_bitmap(c, false));
        }
        for c in 32..64 {
            bitmaps[0xa0 + (c - 32)] = Some(generate_bitmap(c, false));
        }

        // Inverse Text
        for c in 0..32 {
            bitmaps[c] = Some(generate_bitmap(c, true));
        }
        for c in 32..64 {
            bitmaps[0x20 + (c - 32)] = Some(generate_bitmap(c, true));
        }

        // TODO Flashing - Currently simply rendered as inverse
        for c in 0..32 {
            bitmaps[0x40 + c] = Some(generate_bitmap(c, true));
        }
        for c in 32..64 {
            bitmaps[0x60 + (c - 32)] = Some(generate_bitmap(c, true));
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
    /// Apple ][ compatible: upper case and symbols only. No lower case, no
    /// MouseText. `$00-$7F` display inverse (`$40-$7F` flashing), `$80-$FF`
    /// normal.
    Primary,
    /// Adds lower case and MouseText: inverse UC/sym (`$00-$3F`), MouseText
    /// (`$40-$5F`), inverse lower case (`$60-$7F`), normal UC/sym/lower case
    /// (`$80-$FF`).
    Alternate,
}

/// Decode one glyph from the enhanced //e 4K video ROM: eight rows starting
/// at `rom[idx * 8]` — note there is **no** `+1` offset here, unlike the ][+
/// 2716 dump — with seven pixels per row scanned from bit 6 down to bit 0.
fn generate_bitmap_iie(idx: usize, inverse: bool) -> Glyph {
    let mut glyph = [false; CHR_WIDTH * CHR_HEIGHT];
    let mut p = 0;
    for y in 0..CHR_HEIGHT {
        let mut row = CHR_ROM_IIE[(idx * 8) + y];
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

/// Primary-set screen code → (ROM glyph index, inverse?). Every code resolves
/// to one of the 64 upper-case/symbol glyphs at ROM `$00-$3F`; the top bit
/// selects normal vs inverse. Flashing codes (`$40-$7F`) are rendered in
/// their inverse phase, matching the ][+ decode above.
fn primary_index(code: u8) -> (usize, bool) {
    ((code & 0x3f) as usize, code < 0x80)
}

/// Alternate-set screen code → (ROM glyph index, inverse?). This is the //e
/// video display-code translation, derived from the ROM layout: MouseText is
/// read straight from ROM `$40-$5F`; lower case from ROM `$60-$7F`.
fn alternate_index(code: u8) -> (usize, bool) {
    match code {
        0x00..=0x3f => ((code & 0x3f) as usize, true), // inverse UC / symbols
        0x40..=0x5f => (code as usize, false),         // MouseText (as stored)
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
    pub fn new() -> ChrE {
        let mut sets = [[[false; CHR_WIDTH * CHR_HEIGHT]; 256]; 2];
        for (code, glyph) in sets[CharSet::Primary as usize].iter_mut().enumerate() {
            let (idx, inverse) = primary_index(code as u8);
            *glyph = generate_bitmap_iie(idx, inverse);
        }
        for (code, glyph) in sets[CharSet::Alternate as usize].iter_mut().enumerate() {
            let (idx, inverse) = alternate_index(code as u8);
            *glyph = generate_bitmap_iie(idx, inverse);
        }
        ChrE { sets }
    }

    /// The glyph for a screen code in the given character set.
    pub fn bitmap(&self, set: CharSet, screen_code: u8) -> &Glyph {
        &self.sets[set as usize][screen_code as usize]
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
    fn iie_primary_has_no_lower_case() {
        // The primary set is Apple ][ compatible: $E1 shows the symbol '!'
        // ($E1 & $3F = $21), not lower-case 'a'. It must differ from the
        // alternate set's lower-case 'a' at the same code.
        let chr = ChrE::new();
        assert_eq!(
            chr.bitmap(CharSet::Primary, 0xe1),
            chr.bitmap(CharSet::Primary, 0xa1) // both '!'
        );
        assert_ne!(
            chr.bitmap(CharSet::Primary, 0xe1),
            chr.bitmap(CharSet::Alternate, 0xe1)
        );
    }

    #[test]
    fn iie_alternate_lower_case_a() {
        // Lower-case 'a' is screen code $E1 in the alternate set — the
        // capability the ][+ / primary sets cannot render.
        let chr = ChrE::new();
        let expected = "\
.......
.......
..XXX..
.X.....
.XXXX..
.X...X.
.XXXX..
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
