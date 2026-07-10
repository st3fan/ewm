//! Disk activity LED overlay: two small filled circles, one per Disk II
//! drive, rendered into a tiny transparent-background pixel strip. The
//! frontend uploads it to a `BlendMode::Blend` texture and copies it at 3x
//! into the lower-right corner of the screen — only while a drive is lit,
//! so the overlay is invisible when both drives are idle. Rendering is pure
//! (no SDL) so it is unit-testable headless, like `scr.rs`.

use crate::scr::PixelLayout;

/// Circle diameter in emulated (1x) pixels.
pub const LED_DIAMETER: usize = 7;
/// Gap between the two circles.
pub const LED_GAP: usize = 3;

pub const LED_STRIP_WIDTH: usize = LED_DIAMETER * 2 + LED_GAP;
pub const LED_STRIP_HEIGHT: usize = LED_DIAMETER;

/// Render the two-drive LED strip: a filled circle per drive, red when lit,
/// grey when not, on a transparent background. The caller only draws the
/// strip at all when at least one drive is lit.
pub fn render_led_strip(lit: [bool; 2], layout: PixelLayout) -> Vec<u32> {
    let red = layout.pack(255, 0, 0, 255);
    let grey = layout.pack(128, 128, 128, 255);

    let mut pixels = vec![0u32; LED_STRIP_WIDTH * LED_STRIP_HEIGHT];
    for (drive, &on) in lit.iter().enumerate() {
        let color = if on { red } else { grey };
        let left = drive * (LED_DIAMETER + LED_GAP);
        // Filled circle around the cell center; radius in half-pixel units
        // so a 7px cell lights its full width on the center row.
        let c = (LED_DIAMETER - 1) as i32;
        for y in 0..LED_DIAMETER {
            for x in 0..LED_DIAMETER {
                let dx = 2 * x as i32 - c;
                let dy = 2 * y as i32 - c;
                if dx * dx + dy * dy <= c * c {
                    pixels[y * LED_STRIP_WIDTH + left + x] = color;
                }
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: (u8, u8, u8, u8) = (255, 0, 0, 255);
    const GREY: (u8, u8, u8, u8) = (128, 128, 128, 255);

    fn at(pixels: &[u32], x: usize, y: usize) -> u32 {
        pixels[y * LED_STRIP_WIDTH + x]
    }

    #[test]
    fn strip_has_expected_dimensions() {
        let pixels = render_led_strip([false, false], PixelLayout::Argb8888);
        assert_eq!(pixels.len(), LED_STRIP_WIDTH * LED_STRIP_HEIGHT);
        assert_eq!(LED_STRIP_WIDTH, 17);
        assert_eq!(LED_STRIP_HEIGHT, 7);
    }

    #[test]
    fn circle_centers_carry_the_drive_color() {
        let layout = PixelLayout::Argb8888;
        let red = layout.pack(RED.0, RED.1, RED.2, RED.3);
        let grey = layout.pack(GREY.0, GREY.1, GREY.2, GREY.3);
        let mid = LED_DIAMETER / 2;

        let pixels = render_led_strip([true, false], layout);
        assert_eq!(at(&pixels, mid, mid), red, "drive 1 active is red");
        assert_eq!(
            at(&pixels, LED_DIAMETER + LED_GAP + mid, mid),
            grey,
            "drive 2 idle is grey"
        );

        let pixels = render_led_strip([false, true], layout);
        assert_eq!(at(&pixels, mid, mid), grey, "drive 1 idle is grey");
        assert_eq!(
            at(&pixels, LED_DIAMETER + LED_GAP + mid, mid),
            red,
            "drive 2 active is red"
        );
    }

    #[test]
    fn background_and_cell_corners_are_transparent() {
        let pixels = render_led_strip([true, true], PixelLayout::Argb8888);
        // The gap between the circles is untouched.
        let mid = LED_DIAMETER / 2;
        assert_eq!(at(&pixels, LED_DIAMETER + LED_GAP / 2, mid), 0);
        // The corners of each 7x7 cell stay transparent — circles, not
        // squares.
        for left in [0, LED_DIAMETER + LED_GAP] {
            for (x, y) in [(0, 0), (LED_DIAMETER - 1, 0), (0, LED_DIAMETER - 1)] {
                assert_eq!(at(&pixels, left + x, y), 0, "corner ({x},{y})");
            }
        }
        // ARGB8888 transparent means alpha 0.
        assert_eq!(pixels[0] >> 24, 0);
    }

    #[test]
    fn both_layouts_pack_red_correctly() {
        let mid = LED_DIAMETER / 2;
        for layout in [
            PixelLayout::Argb8888,
            PixelLayout::Rgba8888,
            PixelLayout::Rgb888,
        ] {
            let pixels = render_led_strip([true, true], layout);
            assert_eq!(
                at(&pixels, mid, mid),
                layout.pack(255, 0, 0, 255),
                "red packs per layout"
            );
        }
    }
}
