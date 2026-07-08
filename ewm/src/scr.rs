//! Apple ][+ screen renderer, port of `scr.c`: TEXT (interleaved rows,
//! flashing), LGR (16 colors), and HGR (green monochrome or NTSC-ish color
//! with the fringing fix from #187), mixed mode, and page 2 — all rendered
//! into a 280×192 pixel buffer. Rendering is pure (no SDL) so the golden
//! screenshot test runs headless; the SDL loop uploads the buffer as a
//! texture.

use crate::chr::{CHR_HEIGHT, CHR_WIDTH, Chr, ChrE};
use crate::two::{GraphicsMode, GraphicsStyle, ScreenMode, ScreenPage, Two, TwoType};

pub const SCR_WIDTH: usize = 280;
pub const SCR_HEIGHT: usize = 192;

/// The //e frame width. The //e renders its 40-column content into the shared
/// 280-wide `pixels` buffer, then pixel-doubles it horizontally into a true
/// 560-wide buffer (Phase 5a). 80-column text (Phase 5b) will draw natively at
/// this width instead of doubling.
pub const SCR_WIDTH_E: usize = SCR_WIDTH * 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Monochrome,
    Color,
}

/// The pixel layouts `ewm_sdl_pixel_format` can pick; used to pack RGBA
/// colors the way `SDL_MapRGBA` would for the renderer's surface format.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PixelLayout {
    Argb8888,
    Rgba8888,
    Rgb888,
}

impl PixelLayout {
    pub fn pack(self, r: u8, g: u8, b: u8, a: u8) -> u32 {
        let (r, g, b, a) = (r as u32, g as u32, b as u32, a as u32);
        match self {
            PixelLayout::Argb8888 => (a << 24) | (r << 16) | (g << 8) | b,
            PixelLayout::Rgba8888 => (r << 24) | (g << 16) | (b << 8) | a,
            PixelLayout::Rgb888 => (r << 16) | (g << 8) | b,
        }
    }
}

static TXT_LINE_OFFSETS: [usize; 24] = [
    0x000, 0x080, 0x100, 0x180, 0x200, 0x280, 0x300, 0x380, 0x028, 0x0a8, 0x128, 0x1a8, 0x228,
    0x2a8, 0x328, 0x3a8, 0x050, 0x0d0, 0x150, 0x1d0, 0x250, 0x2d0, 0x350, 0x3d0,
];

// (r, g, b, a) tables from scr.c.
static LORES_COLORS: [(u8, u8, u8, u8); 16] = [
    (0, 0, 0, 255),       // 0 Black
    (255, 0, 255, 255),   // 1 Magenta
    (0, 0, 204, 255),     // 2 Dark Blue
    (128, 0, 128, 255),   // 3 Purple
    (0, 100, 0, 255),     // 4 Dark Green
    (128, 128, 128, 255), // 5 Grey 1
    (0, 0, 205, 255),     // 6 Medium Blue
    (173, 216, 230, 255), // 7 Light Blue
    (165, 42, 42, 255),   // 8 Brown
    (255, 165, 0, 255),   // 9 Orange
    (211, 211, 211, 255), // 10 Grey 2
    (255, 192, 203, 255), // 11 Pink
    (144, 238, 144, 255), // 12 Light Green
    (255, 255, 0, 255),   // 13 Yellow
    (127, 255, 212, 255), // 14 Aquamarine
    (255, 255, 255, 255), // 15 White
];

static HGR_COLORS1: [(u8, u8, u8, u8); 4] = [
    (0, 0, 0, 255),       // 00 Black
    (0, 249, 0, 255),     // 01 Green
    (255, 64, 255, 255),  // 10 Purple
    (255, 255, 255, 255), // 11 White
];

static HGR_COLORS2: [(u8, u8, u8, u8); 4] = [
    (0, 0, 0, 255),       // 00 Black
    (255, 147, 0, 255),   // 01 Red
    (0, 150, 255, 255),   // 10 Blue
    (255, 255, 255, 255), // 11 White
];

static HGR_PAGE_OFFSETS: [usize; 2] = [0x2000, 0x4000];

static HGR_LINE_OFFSETS: [usize; 192] = [
    0x0000, 0x0400, 0x0800, 0x0c00, 0x1000, 0x1400, 0x1800, 0x1c00, 0x0080, 0x0480, 0x0880, 0x0c80,
    0x1080, 0x1480, 0x1880, 0x1c80, 0x0100, 0x0500, 0x0900, 0x0d00, 0x1100, 0x1500, 0x1900, 0x1d00,
    0x0180, 0x0580, 0x0980, 0x0d80, 0x1180, 0x1580, 0x1980, 0x1d80, 0x0200, 0x0600, 0x0a00, 0x0e00,
    0x1200, 0x1600, 0x1a00, 0x1e00, 0x0280, 0x0680, 0x0a80, 0x0e80, 0x1280, 0x1680, 0x1a80, 0x1e80,
    0x0300, 0x0700, 0x0b00, 0x0f00, 0x1300, 0x1700, 0x1b00, 0x1f00, 0x0380, 0x0780, 0x0b80, 0x0f80,
    0x1380, 0x1780, 0x1b80, 0x1f80, 0x0028, 0x0428, 0x0828, 0x0c28, 0x1028, 0x1428, 0x1828, 0x1c28,
    0x00a8, 0x04a8, 0x08a8, 0x0ca8, 0x10a8, 0x14a8, 0x18a8, 0x1ca8, 0x0128, 0x0528, 0x0928, 0x0d28,
    0x1128, 0x1528, 0x1928, 0x1d28, 0x01a8, 0x05a8, 0x09a8, 0x0da8, 0x11a8, 0x15a8, 0x19a8, 0x1da8,
    0x0228, 0x0628, 0x0a28, 0x0e28, 0x1228, 0x1628, 0x1a28, 0x1e28, 0x02a8, 0x06a8, 0x0aa8, 0x0ea8,
    0x12a8, 0x16a8, 0x1aa8, 0x1ea8, 0x0328, 0x0728, 0x0b28, 0x0f28, 0x1328, 0x1728, 0x1b28, 0x1f28,
    0x03a8, 0x07a8, 0x0ba8, 0x0fa8, 0x13a8, 0x17a8, 0x1ba8, 0x1fa8, 0x0050, 0x0450, 0x0850, 0x0c50,
    0x1050, 0x1450, 0x1850, 0x1c50, 0x00d0, 0x04d0, 0x08d0, 0x0cd0, 0x10d0, 0x14d0, 0x18d0, 0x1cd0,
    0x0150, 0x0550, 0x0950, 0x0d50, 0x1150, 0x1550, 0x1950, 0x1d50, 0x01d0, 0x05d0, 0x09d0, 0x0dd0,
    0x11d0, 0x15d0, 0x19d0, 0x1dd0, 0x0250, 0x0650, 0x0a50, 0x0e50, 0x1250, 0x1650, 0x1a50, 0x1e50,
    0x02d0, 0x06d0, 0x0ad0, 0x0ed0, 0x12d0, 0x16d0, 0x1ad0, 0x1ed0, 0x0350, 0x0750, 0x0b50, 0x0f50,
    0x1350, 0x1750, 0x1b50, 0x1f50, 0x03d0, 0x07d0, 0x0bd0, 0x0fd0, 0x13d0, 0x17d0, 0x1bd0, 0x1fd0,
];

pub struct Scr {
    color_scheme: ColorScheme,
    chr: Chr,
    chre: ChrE,
    text_color: u32,
    lgr_bitmaps: Vec<[u32; CHR_WIDTH * CHR_HEIGHT]>, // 256 blocks
    pub pixels: Vec<u32>,
    /// The 560-wide //e output, pixel-doubled from `pixels` (Phase 5a). Only
    /// filled when rendering the //e; the ][+ ignores it.
    wide: Vec<u32>,
    green: u32,
    white: u32,
    hgr_colors1: [u32; 4],
    hgr_colors2: [u32; 4],
    /// The 16 lo-res colors packed, reused as the //e double-hi-res palette
    /// (Phase 6b): a 4-bit DHGR cell selects one of these.
    lores_palette: [u32; 16],
}

impl Scr {
    /// Port of `ewm_scr_init`.
    pub fn new(layout: PixelLayout) -> Scr {
        let mut lgr_bitmaps = Vec::with_capacity(256);
        for c in 0..=255usize {
            let mut block = [0u32; CHR_WIDTH * CHR_HEIGHT];
            let (r, g, b, a) = LORES_COLORS[c & 0x0f];
            block[..CHR_WIDTH * 4].fill(layout.pack(r, g, b, a));
            let (r, g, b, a) = LORES_COLORS[(c & 0xf0) >> 4];
            block[CHR_WIDTH * 4..].fill(layout.pack(r, g, b, a));
            lgr_bitmaps.push(block);
        }

        let green = layout.pack(0, 255, 0, 255);
        let white = layout.pack(255, 255, 255, 255);

        let mut lores_palette = [0u32; 16];
        for (dst, &(r, g, b, a)) in lores_palette.iter_mut().zip(LORES_COLORS.iter()) {
            *dst = layout.pack(r, g, b, a);
        }

        let pack4 = |colors: &[(u8, u8, u8, u8); 4]| {
            let mut packed = [0u32; 4];
            for (dst, &(r, g, b, a)) in packed.iter_mut().zip(colors.iter()) {
                *dst = layout.pack(r, g, b, a);
            }
            packed
        };

        Scr {
            color_scheme: ColorScheme::Monochrome,
            chr: Chr::new(),
            chre: ChrE::new(),
            text_color: green,
            lgr_bitmaps,
            pixels: vec![0; SCR_WIDTH * SCR_HEIGHT],
            wide: vec![0; SCR_WIDTH_E * SCR_HEIGHT],
            green,
            white,
            hgr_colors1: pack4(&HGR_COLORS1),
            hgr_colors2: pack4(&HGR_COLORS2),
            lores_palette,
        }
    }

    /// The character generator, shared with the status bar renderer.
    pub fn chr(&self) -> &Chr {
        &self.chr
    }

    /// Port of `ewm_scr_set_color_scheme`: in color mode text renders
    /// white, in monochrome it renders green.
    pub fn set_color_scheme(&mut self, color_scheme: ColorScheme) {
        self.color_scheme = color_scheme;
        self.text_color = if color_scheme == ColorScheme::Monochrome {
            self.green
        } else {
            self.white
        };
    }

    fn text_base(two: &Two) -> usize {
        if two.screen_page() == ScreenPage::Page1 {
            0x0400
        } else {
            0x0800
        }
    }

    /// Port of `scr_render_character`. On the //e the glyph comes from the
    /// enhanced character set selected by ALTCHARSET, so lower case and
    /// MouseText render; the ][+ uses its own set (unmapped codes leave the
    /// buffer untouched, as in C).
    fn render_character(&mut self, two: &Two, row: usize, column: usize, flash: bool) {
        let c = two.ram()[TXT_LINE_OFFSETS[row] + Self::text_base(two) + column];
        let glyph: &[bool] = if two.model() == TwoType::Apple2E {
            self.chre.glyph(two.alt_charset(), c)
        } else {
            match self.chr.bitmap(c) {
                Some(glyph) => glyph,
                None => return,
            }
        };
        let base = (SCR_WIDTH * CHR_HEIGHT * row) + (CHR_WIDTH * column);
        for y in 0..CHR_HEIGHT {
            for x in 0..CHR_WIDTH {
                let dst = &mut self.pixels[base + y * SCR_WIDTH + x];
                if (0x40..0x80).contains(&c) && flash {
                    *dst = 0;
                } else {
                    *dst = if glyph[y * CHR_WIDTH + x] {
                        self.text_color
                    } else {
                        0
                    };
                }
            }
        }
    }

    fn render_txt_screen(&mut self, two: &Two, flash: bool) {
        for row in 0..24 {
            for column in 0..40 {
                self.render_character(two, row, column, flash);
            }
        }
    }

    /// Port of `scr_render_lores_block`.
    fn render_lores_block(&mut self, two: &Two, row: usize, column: usize) {
        let c = two.ram()[TXT_LINE_OFFSETS[row] + Self::text_base(two) + column];
        let block = &self.lgr_bitmaps[c as usize];
        let base = (SCR_WIDTH * CHR_HEIGHT * row) + (CHR_WIDTH * column);
        for y in 0..CHR_HEIGHT {
            for x in 0..CHR_WIDTH {
                self.pixels[base + y * SCR_WIDTH + x] = block[y * CHR_WIDTH + x];
            }
        }
    }

    fn render_lgr_screen(&mut self, two: &Two, flash: bool) {
        let mixed = two.screen_graphics_style() == GraphicsStyle::Mixed;

        // Render graphics
        let rows = if mixed { 20 } else { 24 };
        for row in 0..rows {
            for column in 0..40 {
                self.render_lores_block(two, row, column);
            }
        }

        // Render bottom 4 lines
        if mixed {
            for row in 20..24 {
                for column in 0..40 {
                    self.render_character(two, row, column, flash);
                }
            }
        }
    }

    /// Port of `scr_render_hgr_line_green`.
    fn render_hgr_line_green(&mut self, two: &Two, line: usize, line_base: usize) {
        let src = &two.ram()[line_base..line_base + 40];
        let dst = &mut self.pixels[SCR_WIDTH * line..];
        for (i, &c) in src.iter().enumerate() {
            for j in 0..7 {
                dst[i * 7 + j] = if c & (1 << j) != 0 { self.green } else { 0 };
            }
        }
    }

    /// Port of `scr_render_hgr_line_color`, including the adjacent-pixel
    /// white detection and column-parity coloring from #187.
    fn render_hgr_line_color(&mut self, two: &Two, line: usize, line_base: usize) {
        let mem = &two.ram()[line_base..line_base + 40];
        let dst = &mut self.pixels[SCR_WIDTH * line..];

        for col in 0..280 {
            let byte_idx = col / 7;
            let bit_idx = col % 7;
            let data = mem[byte_idx];
            let high_bit = (data >> 7) & 1;
            let pixel_on = (data >> bit_idx) & 1;

            let colors = if high_bit != 0 {
                &self.hgr_colors2
            } else {
                &self.hgr_colors1
            };

            if pixel_on == 0 {
                dst[col] = 0;
                continue;
            }

            // Check adjacent pixels for white detection
            let mut left_on = 0;
            let mut right_on = 0;

            if col > 0 {
                let left_byte = (col - 1) / 7;
                let left_bit = (col - 1) % 7;
                left_on = (mem[left_byte] >> left_bit) & 1;
            }
            if col < 279 {
                let right_byte = (col + 1) / 7;
                let right_bit = (col + 1) % 7;
                right_on = (mem[right_byte] >> right_bit) & 1;
            }

            if left_on != 0 || right_on != 0 {
                dst[col] = colors[3]; // White
            } else {
                // Isolated pixel - color depends on column parity
                // Even column (0,2,4...) = Violet/Blue (index 2)
                // Odd column (1,3,5...) = Green/Orange (index 1)
                dst[col] = colors[if col & 1 != 0 { 1 } else { 2 }];
            }
        }
    }

    // The line loop deliberately mirrors scr_render_hgr_screen.
    #[allow(clippy::needless_range_loop)]
    fn render_hgr_screen(&mut self, two: &Two, flash: bool) {
        // Render graphics
        let lines = if two.screen_graphics_style() == GraphicsStyle::Mixed {
            160
        } else {
            192
        };
        let hgr_base = HGR_PAGE_OFFSETS[if two.screen_page() == ScreenPage::Page1 {
            0
        } else {
            1
        }];
        for line in 0..lines {
            let line_base = hgr_base + HGR_LINE_OFFSETS[line];
            if self.color_scheme == ColorScheme::Color {
                self.render_hgr_line_color(two, line, line_base);
            } else {
                self.render_hgr_line_green(two, line, line_base);
            }
        }

        // Render bottom 4 lines of text
        if two.screen_graphics_style() == GraphicsStyle::Mixed {
            for row in 20..24 {
                for column in 0..40 {
                    self.render_character(two, row, column, flash);
                }
            }
        }
    }

    /// Port of `ewm_scr_update` (minus the SDL clear, which the loop does).
    pub fn update(&mut self, two: &Two, phase: u32, fps: u32) {
        let flash = !(phase / (fps / 4)).is_multiple_of(2);

        // Some //e modes render natively into the 560-wide buffer; every other
        // mode renders 280-wide and (on the //e) is pixel-doubled.
        if two.model() == TwoType::Apple2E {
            if two.col80() && two.screen_mode() == ScreenMode::Text {
                self.render_txt_screen_80(two, flash);
                return;
            }
            // Double-res graphics, enabled by DHIRES + 80COL: lo-res -> DLGR,
            // hi-res -> DHGR.
            if two.dhires() && two.col80() && two.screen_mode() == ScreenMode::Graphics {
                match two.screen_graphics_mode() {
                    GraphicsMode::Lgr => self.render_dlgr_screen(two, flash),
                    GraphicsMode::Hgr => self.render_dhgr_screen(two, flash),
                }
                return;
            }
        }

        match two.screen_mode() {
            ScreenMode::Text => self.render_txt_screen(two, flash),
            ScreenMode::Graphics => match two.screen_graphics_mode() {
                GraphicsMode::Lgr => self.render_lgr_screen(two, flash),
                GraphicsMode::Hgr => self.render_hgr_screen(two, flash),
            },
        }

        // The //e presents a 560-wide frame; at 40 columns it is the 280-wide
        // render pixel-doubled horizontally.
        if two.model() == TwoType::Apple2E {
            self.fill_wide();
        }
    }

    /// Render the //e 80-column text screen directly into `wide` (560). The two
    /// banks interleave: aux supplies the even display columns (0, 2, …, 78),
    /// main the odd (1, 3, …, 79), each contributing byte `base + column/2` of
    /// the row. Each character is a full 7 px wide (no doubling). Glyphs come
    /// from the ALTCHARSET-selected set, so lower case and MouseText render.
    fn render_txt_screen_80(&mut self, two: &Two, flash: bool) {
        let main = two.ram();
        let aux = two.aux_ram();
        let alt = two.alt_charset();
        for (row, &line_offset) in TXT_LINE_OFFSETS.iter().enumerate() {
            self.render_txt_row_80(main, aux, alt, row, line_offset, flash);
        }
    }

    /// Render one 80-column text row into `wide`: aux supplies the even display
    /// columns, main the odd, each byte `base + column/2` of the row.
    fn render_txt_row_80(
        &mut self,
        main: &[u8],
        aux: &[u8],
        alt: bool,
        row: usize,
        line_offset: usize,
        flash: bool,
    ) {
        let base = 0x400 + line_offset;
        for column in 0..80 {
            let bank = if column % 2 == 0 { aux } else { main };
            let c = bank[base + column / 2];
            let glyph = self.chre.glyph(alt, c);
            let pos = (SCR_WIDTH_E * CHR_HEIGHT * row) + (CHR_WIDTH * column);
            for y in 0..CHR_HEIGHT {
                for x in 0..CHR_WIDTH {
                    let dst = &mut self.wide[pos + y * SCR_WIDTH_E + x];
                    *dst = if (0x40..0x80).contains(&c) && flash {
                        0
                    } else if glyph[y * CHR_WIDTH + x] {
                        self.text_color
                    } else {
                        0
                    };
                }
            }
        }
    }

    /// Render //e double lo-res into `wide` (560): 80 lo-res columns, aux even /
    /// main odd, each a 7px-wide LGR block (reusing the LGR color table). Mixed
    /// mode renders 80-column text in the bottom four rows.
    fn render_dlgr_screen(&mut self, two: &Two, flash: bool) {
        let main = two.ram();
        let aux = two.aux_ram();
        let mixed = two.screen_graphics_style() == GraphicsStyle::Mixed;
        let gfx_rows = if mixed { 20 } else { 24 };
        for (row, &line_offset) in TXT_LINE_OFFSETS.iter().enumerate().take(gfx_rows) {
            let base = 0x400 + line_offset;
            for column in 0..80 {
                let bank = if column % 2 == 0 { aux } else { main };
                let c = bank[base + column / 2];
                let block = &self.lgr_bitmaps[c as usize];
                let pos = (SCR_WIDTH_E * CHR_HEIGHT * row) + (CHR_WIDTH * column);
                for y in 0..CHR_HEIGHT {
                    for x in 0..CHR_WIDTH {
                        self.wide[pos + y * SCR_WIDTH_E + x] = block[y * CHR_WIDTH + x];
                    }
                }
            }
        }
        if mixed {
            let alt = two.alt_charset();
            for (row, &line_offset) in TXT_LINE_OFFSETS.iter().enumerate().skip(20) {
                self.render_txt_row_80(main, aux, alt, row, line_offset, flash);
            }
        }
    }

    /// Render //e double hi-res into `wide` (560). Hi-res page 1 in both banks
    /// interleaves: aux supplies the even 7-pixel groups (0, 2, …), main the
    /// odd, each byte's low 7 bits with bit 0 leftmost (bit 7 is ignored). In
    /// monochrome each of the 560 bits is a pixel; in colour the bit stream is
    /// grouped into aligned 4-bit cells, each selecting one of the 16 lo-res
    /// colours and drawn 4 px wide.
    ///
    /// NOTE (Phase 6b): the colour path uses *aligned* 4-bit cells with the
    /// leftmost bit as the least-significant. This is the simple, deterministic
    /// convention; we may switch to a sliding 4-bit window (closer to NTSC
    /// fringing) after visual review — see the plan doc.
    fn render_dhgr_screen(&mut self, two: &Two, flash: bool) {
        let main = two.ram();
        let aux = two.aux_ram();
        let mixed = two.screen_graphics_style() == GraphicsStyle::Mixed;
        let lines = if mixed { 160 } else { 192 };
        let color = self.color_scheme == ColorScheme::Color;
        for (line, &line_off) in HGR_LINE_OFFSETS.iter().enumerate().take(lines) {
            let line_base = HGR_PAGE_OFFSETS[0] + line_off; // page 1
            // Assemble the 560-bit line: aux even 7-px groups, main odd.
            let mut bits = [false; SCR_WIDTH_E];
            for group in 0..80 {
                let bank = if group % 2 == 0 { aux } else { main };
                let byte = bank[line_base + group / 2];
                for b in 0..7 {
                    bits[group * 7 + b] = (byte >> b) & 1 != 0;
                }
            }
            let row = SCR_WIDTH_E * line;
            if color {
                for cell in 0..SCR_WIDTH_E / 4 {
                    let x = cell * 4;
                    let v = (bits[x] as usize)
                        | (bits[x + 1] as usize) << 1
                        | (bits[x + 2] as usize) << 2
                        | (bits[x + 3] as usize) << 3;
                    let c = self.lores_palette[v];
                    for p in 0..4 {
                        self.wide[row + x + p] = c;
                    }
                }
            } else {
                for (x, &on) in bits.iter().enumerate() {
                    self.wide[row + x] = if on { self.green } else { 0 };
                }
            }
        }
        if mixed {
            let alt = two.alt_charset();
            for (r, &line_offset) in TXT_LINE_OFFSETS.iter().enumerate().skip(20) {
                self.render_txt_row_80(main, aux, alt, r, line_offset, flash);
            }
        }
    }

    /// Pixel-double `pixels` (280) into `wide` (560), horizontally.
    fn fill_wide(&mut self) {
        for y in 0..SCR_HEIGHT {
            let src = &self.pixels[y * SCR_WIDTH..(y + 1) * SCR_WIDTH];
            let dst = &mut self.wide[y * SCR_WIDTH_E..(y + 1) * SCR_WIDTH_E];
            for (x, &p) in src.iter().enumerate() {
                dst[x * 2] = p;
                dst[x * 2 + 1] = p;
            }
        }
    }

    /// The frame buffer to display or capture for `model`: the 560-wide //e
    /// buffer, or the 280-wide ][+ buffer.
    pub fn frame(&self, model: TwoType) -> &[u32] {
        if model == TwoType::Apple2E {
            &self.wide
        } else {
            &self.pixels
        }
    }
}

/// The frame width for `model`: 560 for the //e, 280 for the ][+.
pub fn frame_width(model: TwoType) -> usize {
    if model == TwoType::Apple2E {
        SCR_WIDTH_E
    } else {
        SCR_WIDTH
    }
}

/// Encode the pixel buffer as a 24-bit BMP (used by the hidden
/// `--screenshot` flag and the golden screenshot test). Pixels must be
/// ARGB8888.
pub fn encode_bmp(pixels: &[u32], width: usize, height: usize) -> Vec<u8> {
    let row_size = (width * 3).div_ceil(4) * 4;
    let data_size = row_size * height;
    let file_size = 54 + data_size;

    let mut bmp = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    // BITMAPINFOHEADER
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]); // BI_RGB
    bmp.extend_from_slice(&(data_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 16]);

    for y in (0..height).rev() {
        let row_start = bmp.len();
        for x in 0..width {
            let p = pixels[y * width + x];
            bmp.push(p as u8); // B
            bmp.push((p >> 8) as u8); // G
            bmp.push((p >> 16) as u8); // R
        }
        while bmp.len() - row_start < row_size {
            bmp.push(0);
        }
    }
    bmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::two::TwoType;

    /// The Phase 7 automated gate: boot the System Master for a fixed
    /// number of cycles (the emulator is deterministic), render the text
    /// screen, and compare against the checked-in golden BMP.
    #[test]
    fn boot_screen_matches_golden_bmp() {
        let mut two = Two::new(TwoType::Apple2Plus).unwrap();
        two.load_disk(
            0,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../disks/DOS33-SystemMaster.dsk"
            ),
        )
        .unwrap();
        two.cpu.reset();

        let mut done = 0u64;
        while done < 100_000_000 {
            done += two.cpu.step() as u64;
        }
        assert!(
            two.text_screen().contains("DOS VERSION 3.3"),
            "System Master did not finish booting; screen was:\n{}",
            two.text_screen()
        );

        let mut scr = Scr::new(PixelLayout::Argb8888);
        scr.update(&two, 0, 40);
        let bmp = encode_bmp(&scr.pixels, SCR_WIDTH, SCR_HEIGHT);

        let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/two-boot.bmp");
        if std::env::var("EWM_WRITE_GOLDEN").is_ok() {
            std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/golden")).unwrap();
            std::fs::write(golden_path, &bmp).unwrap();
            return;
        }
        match std::fs::read(golden_path) {
            Ok(golden) => assert_eq!(bmp, golden, "boot screen differs from the golden BMP"),
            Err(_) => panic!(
                "golden BMP missing — generate it with:\n  \
                 EWM_WRITE_GOLDEN=1 cargo test -p ewm boot_screen_matches_golden_bmp"
            ),
        }
    }

    /// The intermediate //e gate: boot the //e (40-column text) and render it
    /// through the model-aware `Scr` at 280×192, comparing against the golden.
    /// 80-column and double-res are Phase 5/6; this covers the 40-column path.
    #[test]
    fn iie_boot_screen_matches_golden_bmp() {
        let mut two = Two::new(TwoType::Apple2E).unwrap();
        two.load_disk(
            0,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../disks/DOS33-SystemMaster.dsk"
            ),
        )
        .unwrap();
        two.cpu.reset();

        let mut done = 0u64;
        while done < 100_000_000 {
            done += two.cpu.step() as u64;
        }
        assert!(
            two.text_screen().contains("DOS VERSION 3.3"),
            "//e System Master did not finish booting; screen was:\n{}",
            two.text_screen()
        );

        // The //e presents a 560-wide frame (Phase 5a): the 40-column render
        // pixel-doubled horizontally.
        let mut scr = Scr::new(PixelLayout::Argb8888);
        scr.update(&two, 0, 40);
        let bmp = encode_bmp(scr.frame(TwoType::Apple2E), SCR_WIDTH_E, SCR_HEIGHT);

        let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/two-e-40col.bmp");
        if std::env::var("EWM_WRITE_GOLDEN").is_ok() {
            std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/golden")).unwrap();
            std::fs::write(golden_path, &bmp).unwrap();
            return;
        }
        match std::fs::read(golden_path) {
            Ok(golden) => assert_eq!(bmp, golden, "//e boot screen differs from the golden BMP"),
            Err(_) => panic!(
                "golden BMP missing — generate it with:\n  \
                 EWM_WRITE_GOLDEN=1 cargo test -p ewm iie_boot_screen_matches_golden_bmp"
            ),
        }
    }
}
