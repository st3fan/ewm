//! The pure `Vec<u32>` overlay compositor (notes/REMOTE.md §3.1, Phase 1).
//!
//! The passive display overlays — the disk-activity lights and the pause box —
//! are all pure `Vec<u32>` producers (`led.rs`, `tty.rs`). This module
//! composites them onto the machine's screen frame, SDL-free, so **both**
//! frontends show the same thing: the SDL loop uploads the composited frame to
//! one texture, the headless serve loop publishes it to VNC. Before this, the
//! SDL loop layered these as separate SDL textures the VNC path never saw — so
//! a paused machine looked frozen-but-unmarked in a browser, and the LEDs were
//! window-only.
//!
//! Interactive and window-scaling chrome stays a per-frontend concern, by
//! design (notes/REMOTE.md §15): the command palette is SDL-only UI, the
//! status bar is a strip *below* the screen (SDL window geometry), and the
//! scanline effect is a property of the up-scaled image the client does
//! itself. What belongs to the *machine's screen* — LEDs, pause — is shared.

use crate::led::{LED_STRIP_HEIGHT, LED_STRIP_WIDTH, render_led_strip};
use crate::scr::PixelLayout;
use crate::tty::{TTY_PIXEL_HEIGHT, TTY_PIXEL_WIDTH, Tty};

/// The pause overlay to draw, if any.
pub enum Pause {
    /// Running: no overlay.
    Running,
    /// Paused: the plain PAUSED box.
    Paused,
    /// Paused straight from a restored save (notes/STATE.md §6): the box
    /// names the save time.
    Restored(String),
}

/// Which passive overlays to composite this frame.
pub struct Overlays {
    /// Disk-activity lights `[selected, other]`; `None` when the motor is idle
    /// (the strip is not drawn at all).
    pub drive_lights: Option<[bool; 2]>,
    /// The pause overlay.
    pub pause: Pause,
}

/// Composites overlays onto a screen frame, reusing scratch buffers so the
/// per-frame cost is one copy plus the touched overlay pixels. One lives in
/// each frontend's render path.
pub struct Compositor {
    layout: PixelLayout,
    buf: Vec<u32>,
    pause_tty: Tty,
}

/// Logical-pixel inset of the LED strip from the screen's bottom-right corner,
/// matching the SDL loop.
const LED_MARGIN: usize = 4;

impl Compositor {
    pub fn new(layout: PixelLayout) -> Compositor {
        let mut pause_tty = Tty::new(layout.pack(0, 255, 0, 255));
        pause_tty.cursor_enabled = false;
        Compositor {
            layout,
            buf: Vec::new(),
            pause_tty,
        }
    }

    /// Composite `overlays` onto `screen` (`width × height`, row-major),
    /// returning the owned composited frame. With no active overlays this is
    /// `screen` copied verbatim.
    pub fn compose(
        &mut self,
        screen: &[u32],
        width: usize,
        height: usize,
        overlays: &Overlays,
    ) -> &[u32] {
        self.buf.clear();
        self.buf.extend_from_slice(screen);

        if let Some(lit) = overlays.drive_lights
            && (lit[0] || lit[1])
        {
            self.blit_leds(lit, width, height);
        }
        match &overlays.pause {
            Pause::Running => {}
            Pause::Paused => self.blit_pause(None, width, height),
            Pause::Restored(at) => self.blit_pause(Some(at), width, height),
        }
        &self.buf
    }

    /// The disk-activity strip in the lower-right corner (the SDL position, at
    /// native resolution — the whole frame scales together afterwards).
    fn blit_leds(&mut self, lit: [bool; 2], width: usize, height: usize) {
        let strip = render_led_strip(lit, self.layout);
        let x0 = width.saturating_sub(LED_MARGIN + LED_STRIP_WIDTH);
        let y0 = height.saturating_sub(LED_MARGIN + LED_STRIP_HEIGHT);
        for y in 0..LED_STRIP_HEIGHT {
            for x in 0..LED_STRIP_WIDTH {
                // The strip is transparent (0) between the squares.
                let px = strip[y * LED_STRIP_WIDTH + x];
                let (dx, dy) = (x0 + x, y0 + y);
                if px != 0 && dx < width && dy < height {
                    self.buf[dy * width + dx] = px;
                }
            }
        }
    }

    /// Darken the whole screen and stamp the pause box — the pixel-space
    /// equivalent of the SDL pause overlay. The box is a 280-wide TTY frame,
    /// horizontally nearest-scaled to the screen width: identity for the ][+,
    /// 2× for the //e, mirroring the SDL box stretched to fill the screen rect.
    fn blit_pause(&mut self, restored_at: Option<&str>, width: usize, height: usize) {
        for p in self.buf.iter_mut() {
            *p = darken(self.layout, *p);
        }
        self.pause_tty.reset();
        match restored_at {
            Some(at) => {
                self.pause_tty
                    .set_line(7, "      ****************************      ");
                self.pause_tty
                    .set_line(8, "      *                          *      ");
                self.pause_tty
                    .set_line(9, "      * RESTORED FROM SAVE STATE *      ");
                self.pause_tty
                    .set_line(10, &format!("      *{at:^26}*      "));
                self.pause_tty
                    .set_line(11, "      *                          *      ");
                self.pause_tty
                    .set_line(12, "      ****************************      ");
            }
            None => {
                self.pause_tty
                    .set_line(8, "          ********************          ");
                self.pause_tty
                    .set_line(9, "          *                  *          ");
                self.pause_tty
                    .set_line(10, "          * -+-  PAUSED  -+- *          ");
                self.pause_tty
                    .set_line(11, "          *                  *          ");
                self.pause_tty
                    .set_line(12, "          ********************          ");
            }
        }
        self.pause_tty.refresh(0, 0);
        let rows = TTY_PIXEL_HEIGHT.min(height);
        for y in 0..rows {
            for x in 0..width {
                let sx = x * TTY_PIXEL_WIDTH / width; // nearest horizontal scale
                let px = self.pause_tty.pixels[y * TTY_PIXEL_WIDTH + sx];
                if px != 0 {
                    self.buf[y * width + x] = px;
                }
            }
        }
    }
}

/// Darken a packed pixel toward black to ≈1/8 brightness — the pixel-space
/// stand-in for the SDL overlay blending black at alpha 224 (keep 31/255).
fn darken(layout: PixelLayout, p: u32) -> u32 {
    let (r, g, b) = layout.unpack_rgb(p);
    let scale = |c: u8| (c as u16 * 31 / 255) as u8;
    layout.pack(scale(r), scale(g), scale(b), 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 280;
    const H: usize = 192;

    fn blank() -> Vec<u32> {
        vec![PixelLayout::Argb8888.pack(0, 200, 0, 255); W * H]
    }

    #[test]
    fn no_overlays_copies_the_screen_verbatim() {
        let mut c = Compositor::new(PixelLayout::Argb8888);
        let screen = blank();
        let out = c.compose(
            &screen,
            W,
            H,
            &Overlays {
                drive_lights: None,
                pause: Pause::Running,
            },
        );
        assert_eq!(out, &screen[..]);
    }

    #[test]
    fn drive_lights_land_in_the_bottom_right_corner_only() {
        let mut c = Compositor::new(PixelLayout::Argb8888);
        let screen = blank();
        let out = c
            .compose(
                &screen,
                W,
                H,
                &Overlays {
                    drive_lights: Some([true, false]),
                    pause: Pause::Running,
                },
            )
            .to_vec();
        let red = PixelLayout::Argb8888.pack(255, 0, 0, 255);
        // A pixel inside the selected-drive square is red.
        let x = W - LED_MARGIN - LED_STRIP_WIDTH + 1;
        let y = H - LED_MARGIN - LED_STRIP_HEIGHT + 1;
        assert_eq!(out[y * W + x], red);
        // The top-left of the screen is untouched.
        assert_eq!(out[0], screen[0]);
    }

    #[test]
    fn pause_darkens_the_screen_and_draws_the_box() {
        let mut c = Compositor::new(PixelLayout::Argb8888);
        let screen = blank();
        let bright = screen[0];
        let out = c
            .compose(
                &screen,
                W,
                H,
                &Overlays {
                    drive_lights: None,
                    pause: Pause::Paused,
                },
            )
            .to_vec();
        // A corner pixel (outside the box) is darkened, not the original.
        assert_ne!(out[0], bright);
        let (_, g0, _) = PixelLayout::Argb8888.unpack_rgb(bright);
        let (_, g1, _) = PixelLayout::Argb8888.unpack_rgb(out[0]);
        assert!(g1 < g0, "darkened toward black");
        // Somewhere in the box rows there is a bright green box pixel.
        let green = PixelLayout::Argb8888.pack(0, 255, 0, 255);
        let box_pixels = out[9 * 8 * W..13 * 8 * W].iter().filter(|&&p| p == green);
        assert!(box_pixels.count() > 0, "the PAUSED box is drawn");
    }

    #[test]
    fn restored_box_reaches_the_full_width_on_the_iie() {
        // 560-wide //e frame: the box scales to fill, no panic on the wider
        // buffer, and the timestamp text is present.
        let mut c = Compositor::new(PixelLayout::Rgba8888);
        let screen = vec![0u32; 560 * H];
        let out = c.compose(
            &screen,
            560,
            H,
            &Overlays {
                drive_lights: None,
                pause: Pause::Restored("2026-07-18 12:00:00".to_string()),
            },
        );
        assert_eq!(out.len(), 560 * H);
        let green = PixelLayout::Rgba8888.pack(0, 255, 0, 255);
        assert!(out.contains(&green), "box drawn on the //e frame");
    }
}
