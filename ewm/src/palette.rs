//! A VS Code-style command palette, drawn with a real UI font (vendored
//! Inter, rasterized by fontdue) at window-pixel resolution so it reads as
//! native chrome, not emulated screen. Kept free of SDL so the filtering,
//! selection, and rendering logic is unit-testable; the frontend owns the
//! texture and the keymod handling (Cmd-P) and feeds events in as
//! [`PaletteKey`]s and text.

use crate::scr::PixelLayout;
use fontdue::{Font, FontSettings};

static FONT: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");

const COMMANDS: [&str; 5] = [
    "Screenshot",
    "Reset",
    "Full Screen Toggle",
    "Pause",
    "Debugger",
];

// VS Code dark theme colors.
const PANEL_BG: (u8, u8, u8) = (0x25, 0x25, 0x26);
const PANEL_BORDER: (u8, u8, u8) = (0x45, 0x45, 0x45);
const INPUT_BG: (u8, u8, u8) = (0x3c, 0x3c, 0x3c);
const SELECTION_BG: (u8, u8, u8) = (0x04, 0x39, 0x5e);
const TEXT: (u8, u8, u8) = (0xcc, 0xcc, 0xcc);
const PLACEHOLDER: (u8, u8, u8) = (0x80, 0x80, 0x80);
const CURSOR: (u8, u8, u8) = (0xae, 0xaf, 0xad);

const FONT_PX: f32 = 15.0;
const MARGIN: usize = 8;
const INPUT_HEIGHT: usize = 26;
const ROW_HEIGHT: usize = 26;
const ROW_GAP: usize = 6; // between the input box and the first row

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaletteKey {
    Up,
    Down,
    Enter,
    Escape,
    Backspace,
}

/// What the frontend should do after an event was handled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaletteAction {
    None,
    /// Close the palette and resume the machine.
    Dismiss,
    /// Close the palette and run the named command.
    Execute(&'static str),
}

pub struct Palette {
    font: Font,
    filter: String,
    selected: usize,
    pub pixels: Vec<u32>,
    layout: PixelLayout,
}

impl Palette {
    pub const WIDTH: usize = 480;
    pub const HEIGHT: usize =
        1 + MARGIN + INPUT_HEIGHT + ROW_GAP + COMMANDS.len() * ROW_HEIGHT + MARGIN + 1;

    pub fn new(layout: PixelLayout) -> Palette {
        let font = Font::from_bytes(FONT, FontSettings::default()).expect("Failed to parse font");
        Palette {
            font,
            filter: String::new(),
            selected: 0,
            pixels: vec![0; Self::WIDTH * Self::HEIGHT],
            layout,
        }
    }

    /// Reset to the just-opened state.
    pub fn open(&mut self) {
        self.filter.clear();
        self.selected = 0;
    }

    /// The commands matching the filter, in declaration order.
    fn filtered(&self) -> Vec<&'static str> {
        let needle = self.filter.to_lowercase();
        COMMANDS
            .iter()
            .copied()
            .filter(|command| command.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn handle_key(&mut self, key: PaletteKey) -> PaletteAction {
        match key {
            PaletteKey::Escape => return PaletteAction::Dismiss,
            PaletteKey::Up => self.selected = self.selected.saturating_sub(1),
            PaletteKey::Down => {
                let count = self.filtered().len();
                if self.selected + 1 < count {
                    self.selected += 1;
                }
            }
            PaletteKey::Enter => {
                if let Some(command) = self.filtered().get(self.selected) {
                    return PaletteAction::Execute(command);
                }
            }
            PaletteKey::Backspace => {
                self.filter.pop();
                self.selected = 0;
            }
        }
        PaletteAction::None
    }

    pub fn handle_text(&mut self, text: &str) -> PaletteAction {
        self.filter.push_str(text);
        self.selected = 0;
        PaletteAction::None
    }

    /// Render the panel into `pixels`. The panel is fully opaque; glyph
    /// antialiasing is blended in software against the known backgrounds.
    pub fn render(&mut self) {
        let (w, h) = (Self::WIDTH, Self::HEIGHT);

        self.fill_rect(0, 0, w, h, PANEL_BORDER);
        self.fill_rect(1, 1, w - 2, h - 2, PANEL_BG);

        // The filter input box, with a text cursor.
        let input_x = 1 + MARGIN;
        let input_y = 1 + MARGIN;
        let input_w = w - 2 - 2 * MARGIN;
        self.fill_rect(input_x, input_y, input_w, INPUT_HEIGHT, INPUT_BG);
        let text_x = input_x + MARGIN;
        let cursor_x = if self.filter.is_empty() {
            self.draw_text(
                text_x,
                input_y,
                INPUT_HEIGHT,
                "Filter commands",
                PLACEHOLDER,
                INPUT_BG,
            );
            text_x
        } else {
            let filter = self.filter.clone();
            self.draw_text(text_x, input_y, INPUT_HEIGHT, &filter, TEXT, INPUT_BG)
        };
        self.fill_rect(cursor_x + 1, input_y + 4, 1, INPUT_HEIGHT - 8, CURSOR);

        // The command rows.
        let rows_y = input_y + INPUT_HEIGHT + ROW_GAP;
        for (i, command) in self.filtered().into_iter().enumerate() {
            let row_y = rows_y + i * ROW_HEIGHT;
            let bg = if i == self.selected {
                SELECTION_BG
            } else {
                PANEL_BG
            };
            if i == self.selected {
                self.fill_rect(1, row_y, w - 2, ROW_HEIGHT, bg);
            }
            self.draw_text(text_x, row_y, ROW_HEIGHT, command, TEXT, bg);
        }
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: (u8, u8, u8)) {
        let packed = self.layout.pack(color.0, color.1, color.2, 255);
        for row in y..(y + h).min(Self::HEIGHT) {
            for column in x..(x + w).min(Self::WIDTH) {
                self.pixels[row * Self::WIDTH + column] = packed;
            }
        }
    }

    /// Draw `text` vertically centered in a box starting at (`x`, `box_y`)
    /// of height `box_h`, blending glyph coverage against `bg`. Returns the
    /// x position of the pen after the last glyph.
    fn draw_text(
        &mut self,
        x: usize,
        box_y: usize,
        box_h: usize,
        text: &str,
        fg: (u8, u8, u8),
        bg: (u8, u8, u8),
    ) -> usize {
        let line = self
            .font
            .horizontal_line_metrics(FONT_PX)
            .expect("Font has no horizontal metrics");
        // descent is negative; center ascent+descent within the box.
        let baseline =
            box_y as f32 + (box_h as f32 - (line.ascent - line.descent)) / 2.0 + line.ascent;

        let mut pen_x = x as f32;
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, FONT_PX);
            let glyph_x = pen_x as i32 + metrics.xmin;
            let glyph_y = baseline as i32 - (metrics.ymin + metrics.height as i32);
            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let px = glyph_x + gx as i32;
                    let py = glyph_y + gy as i32;
                    if px < 0 || py < 0 || px as usize >= Self::WIDTH || py as usize >= Self::HEIGHT
                    {
                        continue;
                    }
                    let coverage = bitmap[gy * metrics.width + gx] as u32;
                    let blend = |f: u8, b: u8| {
                        ((f as u32 * coverage + b as u32 * (255 - coverage)) / 255) as u8
                    };
                    self.pixels[py as usize * Self::WIDTH + px as usize] = self.layout.pack(
                        blend(fg.0, bg.0),
                        blend(fg.1, bg.1),
                        blend(fg.2, bg.2),
                        255,
                    );
                }
            }
            pen_x += metrics.advance_width;
        }
        pen_x as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::new(PixelLayout::Argb8888)
    }

    #[test]
    fn all_commands_visible_when_filter_empty() {
        let p = palette();
        assert_eq!(p.filtered().len(), COMMANDS.len());
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        let mut p = palette();
        p.handle_text("DEBUG");
        assert_eq!(p.filtered(), vec!["Debugger"]);
        p.open();
        p.handle_text("re"); // "sc*re*enshot", "*re*set", "full sc*re*en toggle"
        assert_eq!(
            p.filtered(),
            vec!["Screenshot", "Reset", "Full Screen Toggle"]
        );
    }

    #[test]
    fn selection_moves_and_clamps() {
        let mut p = palette();
        assert_eq!(p.handle_key(PaletteKey::Up), PaletteAction::None);
        assert_eq!(p.selected, 0, "up from the top stays at the top");
        for _ in 0..10 {
            p.handle_key(PaletteKey::Down);
        }
        assert_eq!(p.selected, COMMANDS.len() - 1, "down clamps at the end");
    }

    #[test]
    fn typing_resets_selection() {
        let mut p = palette();
        p.handle_key(PaletteKey::Down);
        p.handle_text("pause");
        assert_eq!(p.selected, 0);
        assert_eq!(
            p.handle_key(PaletteKey::Enter),
            PaletteAction::Execute("Pause")
        );
    }

    #[test]
    fn enter_on_empty_result_does_nothing() {
        let mut p = palette();
        p.handle_text("zzz");
        assert!(p.filtered().is_empty());
        assert_eq!(p.handle_key(PaletteKey::Enter), PaletteAction::None);
    }

    #[test]
    fn escape_dismisses_and_backspace_edits() {
        let mut p = palette();
        assert_eq!(p.handle_key(PaletteKey::Escape), PaletteAction::Dismiss);
        p.handle_text("res");
        p.handle_key(PaletteKey::Backspace);
        assert_eq!(p.filter, "re");
    }

    #[test]
    fn open_resets_state() {
        let mut p = palette();
        p.handle_text("debug");
        p.handle_key(PaletteKey::Down);
        p.open();
        assert_eq!(p.filter, "");
        assert_eq!(p.selected, 0);
        assert_eq!(p.filtered().len(), COMMANDS.len());
    }

    #[test]
    fn render_produces_a_panel() {
        let mut p = palette();
        p.render();
        assert_eq!(p.pixels.len(), Palette::WIDTH * Palette::HEIGHT);
        // The panel is opaque and non-uniform: border, background, and text
        // must all be present.
        let border = PixelLayout::Argb8888.pack(0x45, 0x45, 0x45, 255);
        let panel = PixelLayout::Argb8888.pack(0x25, 0x25, 0x26, 255);
        assert_eq!(p.pixels[0], border);
        assert!(p.pixels.contains(&panel));
        let distinct: std::collections::HashSet<&u32> = p.pixels.iter().collect();
        assert!(
            distinct.len() > 10,
            "antialiased text should produce many shades"
        );
    }
}
