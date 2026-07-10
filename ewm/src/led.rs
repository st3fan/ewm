//! Disk activity LED overlay: two small filled squares, one per Disk II
//! drive, rendered into a tiny transparent-background pixel strip. The
//! frontend uploads it to a `BlendMode::Blend` texture and copies it at 3x
//! into the lower-right corner of the screen — only while a drive is lit,
//! so the overlay is invisible when both drives are idle. Rendering is pure
//! (no SDL) so it is unit-testable headless, like `scr.rs`.

use crate::scr::PixelLayout;

/// Square side in emulated (1x) pixels.
pub const LED_SIZE: usize = 5;
/// Gap between the two squares.
pub const LED_GAP: usize = 3;

pub const LED_STRIP_WIDTH: usize = LED_SIZE * 2 + LED_GAP;
pub const LED_STRIP_HEIGHT: usize = LED_SIZE;

/// Render the two-drive LED strip: a filled square per drive, red when lit,
/// grey when not, on a transparent background. The caller only draws the
/// strip at all when at least one drive is lit.
pub fn render_led_strip(lit: [bool; 2], layout: PixelLayout) -> Vec<u32> {
    let red = layout.pack(255, 0, 0, 255);
    let grey = layout.pack(128, 128, 128, 255);

    let mut pixels = vec![0u32; LED_STRIP_WIDTH * LED_STRIP_HEIGHT];
    for (drive, &on) in lit.iter().enumerate() {
        let color = if on { red } else { grey };
        let left = drive * (LED_SIZE + LED_GAP);
        for row in pixels.chunks_mut(LED_STRIP_WIDTH) {
            row[left..left + LED_SIZE].fill(color);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(pixels: &[u32], x: usize, y: usize) -> u32 {
        pixels[y * LED_STRIP_WIDTH + x]
    }

    #[test]
    fn strip_has_expected_dimensions() {
        let pixels = render_led_strip([false, false], PixelLayout::Argb8888);
        assert_eq!(pixels.len(), LED_STRIP_WIDTH * LED_STRIP_HEIGHT);
        assert_eq!(LED_STRIP_WIDTH, 13);
        assert_eq!(LED_STRIP_HEIGHT, 5);
    }

    #[test]
    fn squares_carry_the_drive_color() {
        let layout = PixelLayout::Argb8888;
        let red = layout.pack(255, 0, 0, 255);
        let grey = layout.pack(128, 128, 128, 255);

        let pixels = render_led_strip([true, false], layout);
        // Every pixel of each square carries its drive's color, corners
        // included — squares, not circles.
        for y in 0..LED_SIZE {
            for x in 0..LED_SIZE {
                assert_eq!(at(&pixels, x, y), red, "drive 1 active ({x},{y})");
                assert_eq!(
                    at(&pixels, LED_SIZE + LED_GAP + x, y),
                    grey,
                    "drive 2 idle ({x},{y})"
                );
            }
        }

        let pixels = render_led_strip([false, true], layout);
        assert_eq!(at(&pixels, 0, 0), grey, "drive 1 idle is grey");
        assert_eq!(
            at(&pixels, LED_SIZE + LED_GAP, 0),
            red,
            "drive 2 active is red"
        );
    }

    #[test]
    fn gap_between_the_squares_is_transparent() {
        let pixels = render_led_strip([true, true], PixelLayout::Argb8888);
        for y in 0..LED_STRIP_HEIGHT {
            for x in LED_SIZE..LED_SIZE + LED_GAP {
                assert_eq!(at(&pixels, x, y), 0, "gap ({x},{y})");
            }
        }
        // ARGB8888 transparent means alpha 0.
        assert_eq!(at(&pixels, LED_SIZE, 0) >> 24, 0);
    }

    #[test]
    fn both_layouts_pack_red_correctly() {
        for layout in [
            PixelLayout::Argb8888,
            PixelLayout::Rgba8888,
            PixelLayout::Rgb888,
        ] {
            let pixels = render_led_strip([true, true], layout);
            assert_eq!(
                at(&pixels, 0, 0),
                layout.pack(255, 0, 0, 255),
                "red packs per layout"
            );
        }
    }
}
