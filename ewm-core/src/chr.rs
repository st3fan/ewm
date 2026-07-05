//! Character ROM decoding, port of the bitmap half of `chr.c`: the 2716
//! character ROM (`3410036.bin`) becomes per-character 7×8 glyph bitmaps,
//! indexed by Apple ][ screen code. Texture creation from these bitmaps is
//! frontend work (Phase 7); nothing here touches SDL.

pub const CHR_WIDTH: usize = 7;
pub const CHR_HEIGHT: usize = 8;

static CHR_ROM: &[u8; 2048] = include_bytes!("../../rom/3410036.bin");

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
}
