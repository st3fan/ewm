//! Terminal display for the Apple 1, port of `tty.c`: a 24×40 screen buffer
//! rendered glyph by glyph into a 280×192 pixel buffer the SDL loop uploads
//! as a texture each refresh.

use ewm_core::chr::{CHR_HEIGHT, CHR_WIDTH, Chr};

pub const TTY_ROWS: usize = 24;
pub const TTY_COLUMNS: usize = 40;
pub const TTY_CURSOR_ON: u8 = b'@';
pub const TTY_CURSOR_OFF: u8 = b' ';

pub const TTY_PIXEL_WIDTH: usize = TTY_COLUMNS * CHR_WIDTH; // 280
pub const TTY_PIXEL_HEIGHT: usize = TTY_ROWS * CHR_HEIGHT; // 192

pub struct Tty {
    chr: Chr,
    pub screen_dirty: bool,
    screen_buffer: [u8; TTY_ROWS * TTY_COLUMNS],
    cursor_row: usize,
    cursor_column: usize,
    cursor_blink: bool,
    pub pixels: Vec<u32>,
    color: u32,
}

impl Tty {
    pub fn new(color: u32) -> Tty {
        let mut tty = Tty {
            chr: Chr::new(),
            screen_dirty: true,
            screen_buffer: [0; TTY_ROWS * TTY_COLUMNS],
            cursor_row: 0,
            cursor_column: 0,
            cursor_blink: false,
            pixels: vec![0; TTY_PIXEL_WIDTH * TTY_PIXEL_HEIGHT],
            color,
        };
        tty.reset();
        tty
    }

    /// Port of `ewm_tty_render_character` (the bitmap path).
    fn render_character(&mut self, row: usize, column: usize, c: u8) {
        let c = c.wrapping_add(0x80); // TODO This should not be there really (comment from tty.c)
        let base = (TTY_PIXEL_WIDTH * CHR_HEIGHT * row) + (CHR_WIDTH * column);
        match self.chr.bitmap(c) {
            Some(glyph) => {
                for y in 0..CHR_HEIGHT {
                    for x in 0..CHR_WIDTH {
                        self.pixels[base + y * TTY_PIXEL_WIDTH + x] = if glyph[y * CHR_WIDTH + x] {
                            self.color
                        } else {
                            0
                        };
                    }
                }
            }
            None => {
                for y in 0..CHR_HEIGHT {
                    for x in 0..CHR_WIDTH {
                        self.pixels[base + y * TTY_PIXEL_WIDTH + x] = 0;
                    }
                }
            }
        }
    }

    fn scroll_up(&mut self) {
        self.screen_buffer
            .copy_within(TTY_COLUMNS..TTY_ROWS * TTY_COLUMNS, 0);
        self.screen_buffer[(TTY_ROWS - 1) * TTY_COLUMNS..].fill(0);
    }

    /// Port of `ewm_tty_write`.
    pub fn write(&mut self, v: u8) {
        if v == b'\r' {
            self.cursor_column = 0;
            self.cursor_row += 1;
            if self.cursor_row == TTY_ROWS {
                self.cursor_row = TTY_ROWS - 1;
                self.scroll_up();
            }
        } else {
            self.screen_buffer[(self.cursor_row * TTY_COLUMNS) + self.cursor_column] = v;
            self.cursor_column += 1;
            if self.cursor_column == TTY_COLUMNS {
                self.cursor_column = 0;
                self.cursor_row += 1;
                if self.cursor_row == TTY_ROWS {
                    self.cursor_row = TTY_ROWS - 1;
                    self.scroll_up();
                }
            }
        }
        self.screen_dirty = true;
    }

    pub fn reset(&mut self) {
        self.screen_buffer.fill(0);
        self.cursor_row = 0;
        self.cursor_column = 0;
        self.screen_dirty = true;
    }

    /// Port of `ewm_tty_refresh`: render the buffer and the blinking cursor
    /// into the pixel buffer.
    pub fn refresh(&mut self, phase: u32, fps: u32) {
        for row in 0..TTY_ROWS {
            for column in 0..TTY_COLUMNS {
                let c = self.screen_buffer[(row * TTY_COLUMNS) + column];
                self.render_character(row, column, c);
            }
        }

        if fps != 0 && phase.is_multiple_of(fps / 4) {
            self.cursor_blink = !self.cursor_blink;
        }

        let cursor = if self.cursor_blink {
            TTY_CURSOR_ON
        } else {
            TTY_CURSOR_OFF
        };
        self.render_character(self.cursor_row, self.cursor_column, cursor);
    }
}
