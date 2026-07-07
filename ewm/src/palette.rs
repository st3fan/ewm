//! A VS Code-style command palette, drawn with a real UI font (vendored
//! Inter, rasterized by fontdue) at window-pixel resolution so it reads as
//! native chrome, not emulated screen.
//!
//! The palette itself knows nothing about the machines or SDL: a frontend
//! registers commands each time it opens the palette — so labels can
//! reflect the current state ("Pause" vs "Unpause") — pairing a label with
//! an opaque action value, typically a callback into the frontend
//! (`fn(&mut Ctx)`). When the user picks a command the action is handed
//! back in [`PaletteAction::Execute`] for the frontend to invoke; the
//! palette never calls it. This keeps the module free of SDL and lifetimes,
//! and the filtering, selection, and rendering logic unit-testable.

use crate::scr::PixelLayout;
use fontdue::{Font, FontSettings};

static FONT: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");

/// Texture dimensions: allocate for the widest panel we ever draw. The
/// visible height shrinks with the command list — see [`Palette::height`].
pub const WIDTH: usize = 480;
pub const MAX_HEIGHT: usize =
    1 + MARGIN + INPUT_HEIGHT + ROW_GAP + MAX_ROWS * ROW_HEIGHT + MARGIN + 1;

const MAX_ROWS: usize = 8;

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
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaletteAction<A> {
    None,
    /// Close the palette and resume the machine.
    Dismiss,
    /// Close the palette and invoke the selected command's action.
    Execute(A),
}

pub struct Palette<A> {
    font: Font,
    commands: Vec<(String, A)>,
    filter: String,
    selected: usize,
    pub pixels: Vec<u32>,
    layout: PixelLayout,
}

impl<A: Copy> Palette<A> {
    pub fn new(layout: PixelLayout) -> Palette<A> {
        let font = Font::from_bytes(FONT, FontSettings::default()).expect("Failed to parse font");
        Palette {
            font,
            commands: Vec::new(),
            filter: String::new(),
            selected: 0,
            pixels: vec![0; WIDTH * MAX_HEIGHT],
            layout,
        }
    }

    /// Reset to the just-opened state. The frontend follows up with
    /// `add_command` calls reflecting the machine's current state.
    pub fn open(&mut self) {
        self.commands.clear();
        self.filter.clear();
        self.selected = 0;
    }

    pub fn add_command(&mut self, label: impl Into<String>, action: A) {
        self.commands.push((label.into(), action));
    }

    /// Indices into `commands` matching the filter, in registration order.
    fn filtered(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        self.commands
            .iter()
            .enumerate()
            .filter(|(_, (label, _))| label.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    /// The visible panel height for the current filter result; the panel
    /// shrinks as the list narrows.
    pub fn height(&self) -> usize {
        let rows = self.filtered().len().min(MAX_ROWS);
        let rows_height = if rows > 0 {
            ROW_GAP + rows * ROW_HEIGHT
        } else {
            0
        };
        1 + MARGIN + INPUT_HEIGHT + rows_height + MARGIN + 1
    }

    pub fn handle_key(&mut self, key: PaletteKey) -> PaletteAction<A> {
        match key {
            PaletteKey::Escape => return PaletteAction::Dismiss,
            PaletteKey::Up => self.selected = self.selected.saturating_sub(1),
            PaletteKey::Down => {
                if self.selected + 1 < self.filtered().len() {
                    self.selected += 1;
                }
            }
            PaletteKey::Enter => {
                if let Some(&index) = self.filtered().get(self.selected) {
                    return PaletteAction::Execute(self.commands[index].1);
                }
            }
            PaletteKey::Backspace => {
                self.filter.pop();
                self.selected = 0;
            }
        }
        PaletteAction::None
    }

    pub fn handle_text(&mut self, text: &str) -> PaletteAction<A> {
        self.filter.push_str(text);
        self.selected = 0;
        PaletteAction::None
    }

    /// Render the panel into the top `height()` rows of `pixels`. The panel
    /// is fully opaque; glyph antialiasing is blended in software against
    /// the known backgrounds.
    pub fn render(&mut self) {
        let (w, h) = (WIDTH, self.height());

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
        for (row, index) in self.filtered().into_iter().take(MAX_ROWS).enumerate() {
            let row_y = rows_y + row * ROW_HEIGHT;
            let bg = if row == self.selected {
                SELECTION_BG
            } else {
                PANEL_BG
            };
            if row == self.selected {
                self.fill_rect(1, row_y, w - 2, ROW_HEIGHT, bg);
            }
            let label = self.commands[index].0.clone();
            self.draw_text(text_x, row_y, ROW_HEIGHT, &label, TEXT, bg);
        }
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: (u8, u8, u8)) {
        let packed = self.layout.pack(color.0, color.1, color.2, 255);
        for row in y..(y + h).min(MAX_HEIGHT) {
            for column in x..(x + w).min(WIDTH) {
                self.pixels[row * WIDTH + column] = packed;
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
                    if px < 0 || py < 0 || px as usize >= WIDTH || py as usize >= MAX_HEIGHT {
                        continue;
                    }
                    let coverage = bitmap[gy * metrics.width + gx] as u32;
                    let blend = |f: u8, b: u8| {
                        ((f as u32 * coverage + b as u32 * (255 - coverage)) / 255) as u8
                    };
                    self.pixels[py as usize * WIDTH + px as usize] = self.layout.pack(
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

    /// A palette whose actions are plain markers; frontends use fn pointers.
    fn palette() -> Palette<u32> {
        let mut p = Palette::new(PixelLayout::Argb8888);
        p.open();
        p.add_command("Reset", 1);
        p.add_command("Pause", 2);
        p.add_command("Enter Full Screen", 3);
        p
    }

    #[test]
    fn commands_are_registered_per_open() {
        let mut p = palette();
        assert_eq!(p.filtered().len(), 3);
        p.open();
        assert_eq!(p.filtered().len(), 0, "open() clears the registrations");
        p.add_command("Unpause", 2);
        assert_eq!(p.filtered().len(), 1, "labels can differ per activation");
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        let mut p = palette();
        p.handle_text("SCREEN");
        assert_eq!(p.filtered(), vec![2]); // Enter Full Screen
        p.handle_key(PaletteKey::Backspace);
        assert_eq!(p.filter, "SCREE");
    }

    #[test]
    fn selection_moves_and_clamps() {
        let mut p = palette();
        assert_eq!(p.handle_key(PaletteKey::Up), PaletteAction::None);
        assert_eq!(p.selected, 0, "up from the top stays at the top");
        for _ in 0..10 {
            p.handle_key(PaletteKey::Down);
        }
        assert_eq!(p.selected, 2, "down clamps at the end");
    }

    #[test]
    fn enter_executes_the_selected_action() {
        let mut p = palette();
        p.handle_key(PaletteKey::Down);
        assert_eq!(p.handle_key(PaletteKey::Enter), PaletteAction::Execute(2));
        // Filtering re-anchors the selection to the narrowed list.
        p.handle_text("reset");
        assert_eq!(p.handle_key(PaletteKey::Enter), PaletteAction::Execute(1));
    }

    #[test]
    fn enter_on_empty_result_does_nothing() {
        let mut p = palette();
        p.handle_text("zzz");
        assert!(p.filtered().is_empty());
        assert_eq!(p.handle_key(PaletteKey::Enter), PaletteAction::None);
    }

    #[test]
    fn escape_dismisses() {
        let mut p = palette();
        assert_eq!(p.handle_key(PaletteKey::Escape), PaletteAction::Dismiss);
    }

    #[test]
    fn fn_pointer_actions_round_trip() {
        struct Ctx {
            resets: u32,
        }
        let mut p: Palette<fn(&mut Ctx)> = Palette::new(PixelLayout::Argb8888);
        p.open();
        p.add_command("Reset", |ctx| ctx.resets += 1);
        let mut ctx = Ctx { resets: 0 };
        if let PaletteAction::Execute(run) = p.handle_key(PaletteKey::Enter) {
            run(&mut ctx);
        }
        assert_eq!(ctx.resets, 1);
    }

    #[test]
    fn panel_shrinks_with_the_filter() {
        let mut p = palette();
        let full = p.height();
        p.handle_text("pause");
        assert!(p.height() < full, "fewer rows, shorter panel");
        p.handle_text("zzz");
        assert!(p.height() < full, "no rows at all is shortest");
        assert!(p.height() <= MAX_HEIGHT);
    }

    #[test]
    fn render_produces_a_panel() {
        let mut p = palette();
        p.render();
        assert_eq!(p.pixels.len(), WIDTH * MAX_HEIGHT);
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
