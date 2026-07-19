//! Generate the EWM app icon: `][` rendered by the emulator's own Apple ][
//! character generator (roms/3410036.bin), green phosphor on a dark bezel
//! with rounded (alpha) corners. Writes every size `iconutil` needs into an
//! `EWM.iconset` directory:
//!
//!   cargo run -p ewm --example icon -- dist/EWM.iconset
//!
//! The bundling script (scripts/make-app.sh) then runs
//! `iconutil -c icns` on it. Each size is drawn independently at integer
//! glyph scales, so the pixel art stays crisp instead of being resampled.

use std::io::Write;

use ewm::chr::{CHR_HEIGHT, CHR_WIDTH, Chr};

/// The phosphor green of the default monitor style.
const GREEN: [u8; 4] = [0x33, 0xff, 0x66, 0xff];
/// The bezel.
const DARK: [u8; 4] = [0x1c, 0x1c, 0x1e, 0xff];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// The `iconutil` file set: (basename, pixel size).
const SIZES: [(&str, usize); 10] = [
    ("icon_16x16", 16),
    ("icon_16x16@2x", 32),
    ("icon_32x32", 32),
    ("icon_32x32@2x", 64),
    ("icon_128x128", 128),
    ("icon_128x128@2x", 256),
    ("icon_256x256", 256),
    ("icon_256x256@2x", 512),
    ("icon_512x512", 512),
    ("icon_512x512@2x", 1024),
];

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "EWM.iconset".to_string());
    std::fs::create_dir_all(&out).expect("create iconset dir");
    let chr = Chr::new();
    for (name, size) in SIZES {
        let image = render(&chr, size);
        let png = encode_png(&image, size, size);
        std::fs::write(format!("{out}/{name}.png"), png).expect("write png");
    }
    println!("wrote {}/", out);
}

/// Draw one icon: a rounded dark square with `][` centered in green.
fn render(chr: &Chr, size: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; size * size * 4];
    let radius = size / 5;

    // The bezel, with transparent rounded corners.
    for y in 0..size {
        for x in 0..size {
            let px = if inside_rounded(x, y, size, radius) {
                DARK
            } else {
                CLEAR
            };
            rgba[(y * size + x) * 4..][..4].copy_from_slice(&px);
        }
    }

    // "][" from the character ROM: screen codes $DD (']') then $DB ('[').
    let glyphs = [
        chr.bitmap(0xdd).expect("']' glyph"),
        chr.bitmap(0xdb).expect("'[' glyph"),
    ];
    let art_w = 2 * CHR_WIDTH;
    // The largest integer scale that keeps the art within ~3/4 of the icon.
    let scale = (size * 3 / (4 * art_w)).max(1);
    let x0 = (size - art_w * scale) / 2;
    let y0 = (size - CHR_HEIGHT * scale) / 2;
    for (i, glyph) in glyphs.iter().enumerate() {
        for gy in 0..CHR_HEIGHT {
            for gx in 0..CHR_WIDTH {
                if !glyph[gy * CHR_WIDTH + gx] {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        let x = x0 + (i * CHR_WIDTH + gx) * scale + sx;
                        let y = y0 + gy * scale + sy;
                        rgba[(y * size + x) * 4..][..4].copy_from_slice(&GREEN);
                    }
                }
            }
        }
    }
    rgba
}

/// Is (x, y) inside a size x size square with the given corner radius?
fn inside_rounded(x: usize, y: usize, size: usize, radius: usize) -> bool {
    let (x, y, size, r) = (x as i64, y as i64, size as i64, radius as i64);
    // Distance from the nearest corner circle centre, only in the corner
    // squares; elsewhere we are inside.
    let cx = if x < r {
        r - 1 - x
    } else if x >= size - r {
        x - (size - r)
    } else {
        return true;
    };
    let cy = if y < r {
        r - 1 - y
    } else if y >= size - r {
        y - (size - r)
    } else {
        return true;
    };
    cx * cx + cy * cy <= (r - 1) * (r - 1)
}

/// A minimal PNG writer: 8-bit RGBA, no interlace, filter 0 on every row.
fn encode_png(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    chunk(&mut png, b"IHDR", &ihdr);

    let mut raw = Vec::with_capacity(height * (1 + width * 4));
    for row in rgba.chunks(width * 4) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&raw).unwrap();
    chunk(&mut png, b"IDAT", &z.finish().unwrap());

    chunk(&mut png, b"IEND", &[]);
    png
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(kind);
    crc.update(data);
    png.extend_from_slice(&crc.finish().to_be_bytes());
}

/// CRC-32 (the zlib/PNG polynomial), bitwise — speed is irrelevant here.
struct Crc32(u32);

impl Crc32 {
    fn new() -> Crc32 {
        Crc32(0xffff_ffff)
    }

    fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.0 ^= b as u32;
            for _ in 0..8 {
                let mask = (self.0 & 1).wrapping_neg();
                self.0 = (self.0 >> 1) ^ (0xedb8_8320 & mask);
            }
        }
    }

    fn finish(self) -> u32 {
        !self.0
    }
}
