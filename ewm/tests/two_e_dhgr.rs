//! Enhanced //e double hi-res (Phase 6b). DHGR uses hi-res page 1 in both
//! banks: aux supplies the even 7-pixel groups (0, 2, …), main the odd, each
//! byte's low 7 bits with bit 0 leftmost (bit 7 ignored) → 560 pixels.
//! Monochrome is one pixel per bit (deterministic golden); colour groups the
//! bit stream into aligned 4-bit cells selecting the 16 lo-res colours.

use ewm::scr::{ColorScheme, PixelLayout, SCR_HEIGHT, SCR_WIDTH_E, Scr, encode_bmp};
use ewm::two::{Two, TwoType};

const GRAPHICS: u16 = 0xc050;
const HIRES: u16 = 0xc057;
const COL80_ON: u16 = 0xc00d;
const DHIRES_ON: u16 = 0xc05e; // AN3 off -> DHIRES on
const RAMWRT_OFF: u16 = 0xc004;
const RAMWRT_ON: u16 = 0xc005;
const HGR1: u16 = 0x2000; // hi-res page 1, line 0

const LAYOUT: PixelLayout = PixelLayout::Argb8888;

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}
fn enable_dhgr(two: &mut Two) {
    set(two, GRAPHICS);
    set(two, HIRES);
    set(two, COL80_ON);
    set(two, DHIRES_ON);
}
fn poke_aux(two: &mut Two, addr: u16, b: u8) {
    set(two, RAMWRT_ON);
    two.cpu.mem.write(addr, b);
    set(two, RAMWRT_OFF);
}
fn poke_main(two: &mut Two, addr: u16, b: u8) {
    set(two, RAMWRT_OFF);
    two.cpu.mem.write(addr, b);
}

#[test]
fn aux_is_the_leftmost_group() {
    // aux byte 0 fills display group 0 (pixels 0-6); main byte 0 group 1 (7-13).
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    enable_dhgr(&mut two);
    poke_aux(&mut two, HGR1, 0x7f); // all 7 bits -> pixels 0-6 on
    poke_main(&mut two, HGR1, 0x00); // pixels 7-13 off

    let mut scr = Scr::new(LAYOUT); // monochrome by default
    scr.update(&two, 0, 40);
    let f = scr.frame(TwoType::Apple2EEnhanced);
    let green = LAYOUT.pack(0, 255, 0, 255);
    for (x, &p) in f.iter().take(7).enumerate() {
        assert_eq!(p, green, "pixel {x} (aux group 0) on");
    }
    for (x, &p) in f.iter().take(14).enumerate().skip(7) {
        assert_eq!(p, 0, "pixel {x} (main group 1) off");
    }
}

#[test]
fn bit_zero_is_leftmost() {
    // Only bit 0 set -> only the leftmost pixel of the group lights.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    enable_dhgr(&mut two);
    poke_aux(&mut two, HGR1, 0x01);
    let mut scr = Scr::new(LAYOUT);
    scr.update(&two, 0, 40);
    let f = scr.frame(TwoType::Apple2EEnhanced);
    let green = LAYOUT.pack(0, 255, 0, 255);
    assert_eq!(f[0], green, "bit 0 -> pixel 0");
    assert_eq!(f[1], 0, "bit 1 clear -> pixel 1 off");
}

#[test]
fn colour_cell_selects_the_palette() {
    // A 4-bit cell of value 5 (bits 0 and 2 set) -> lo-res colour 5 (grey),
    // drawn 4 px wide; the next cell (all clear) is black.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    enable_dhgr(&mut two);
    poke_aux(&mut two, HGR1, 0b0000_0101); // bits 0,2 -> cell 0 = 5
    let mut scr = Scr::new(LAYOUT);
    scr.set_color_scheme(ColorScheme::Color);
    scr.update(&two, 0, 40);
    let f = scr.frame(TwoType::Apple2EEnhanced);
    let grey = LAYOUT.pack(128, 128, 128, 255); // lo-res colour 5
    for (x, &p) in f.iter().take(4).enumerate() {
        assert_eq!(p, grey, "cell 0 pixel {x} = colour 5");
    }
    // Cell 1 is colour 0 — palette black (opaque), not the mono off-pixel (0).
    assert_eq!(f[4], LAYOUT.pack(0, 0, 0, 255), "cell 1 = colour 0 (black)");
}

#[test]
fn dhgr_screen_matches_golden_bmp() {
    // Fill hi-res page 1: aux all-on, main all-off -> 40 vertical 7px stripes.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    enable_dhgr(&mut two);
    set(&mut two, RAMWRT_ON);
    for addr in 0x2000u16..0x4000 {
        two.cpu.mem.write(addr, 0x7f);
    }
    set(&mut two, RAMWRT_OFF);
    for addr in 0x2000u16..0x4000 {
        two.cpu.mem.write(addr, 0x00);
    }

    let mut scr = Scr::new(LAYOUT); // monochrome
    scr.update(&two, 0, 40);
    let bmp = encode_bmp(scr.frame(TwoType::Apple2EEnhanced), SCR_WIDTH_E, SCR_HEIGHT);

    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/two-e-dhgr.bmp");
    if std::env::var("EWM_WRITE_GOLDEN").is_ok() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/golden")).unwrap();
        std::fs::write(golden_path, &bmp).unwrap();
        return;
    }
    match std::fs::read(golden_path) {
        Ok(golden) => assert_eq!(bmp, golden, "DHGR screen differs from the golden BMP"),
        Err(_) => panic!(
            "golden BMP missing — generate it with:\n  \
             EWM_WRITE_GOLDEN=1 cargo test -p ewm dhgr_screen_matches_golden_bmp"
        ),
    }
}
