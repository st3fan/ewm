//! Throwaway DHGR colour experiment (not part of the suite): renders a
//! composite test scene in aligned-cell and sliding-window colour modes for
//! visual comparison. Writes BMPs to the path in EWM_DHGR_OUT.

use ewm::scr::{ColorScheme, DhgrColorMode, PixelLayout, SCR_HEIGHT, SCR_WIDTH_E, Scr, encode_bmp};
use ewm::two::{Two, TwoType};

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}

/// Fill one DHGR line from a bit predicate over x (0..560).
fn poke_line(two: &mut Two, line: usize, f: &dyn Fn(usize) -> bool) {
    // Recompute the hi-res line offset the same way scr.rs does.
    let block = line / 64;
    let sub = (line % 64) / 8;
    let row = line % 8;
    let base = 0x2000 + 0x28 * block + 0x80 * sub + 0x400 * row;
    for group in 0..80usize {
        let mut byte = 0u8;
        for b in 0..7 {
            if f(group * 7 + b) {
                byte |= 1 << b;
            }
        }
        // aux = even groups, main = odd.
        set(two, if group % 2 == 0 { 0xc005 } else { 0xc004 });
        two.cpu.mem.write((base + group / 2) as u16, byte);
    }
    set(two, 0xc004); // RAMWRT off
}

fn build_scene(two: &mut Two) {
    // Band 1 (lines 0..48): 32-px colour bars, cell-aligned.
    for line in 0..48 {
        poke_line(two, line, &|x| (x / 32 % 16) & (1 << (x % 4)) != 0);
    }
    // Band 2 (lines 48..80): fine vertical lines, 1 px then 2 px, step 17
    // (so the phase x%4 varies from line group to line group).
    for line in 48..64 {
        poke_line(two, line, &|x| x % 17 == 8);
    }
    for line in 64..80 {
        poke_line(two, line, &|x| x % 17 == 8 || x % 17 == 9);
    }
    // Band 3 (lines 80..112): white blocks — one starting mid-cell (x=13),
    // one cell-aligned (x=200); plus a black gap inside white.
    for line in 80..112 {
        poke_line(two, line, &|x| {
            (13..113).contains(&x) || (200..300).contains(&x) && x != 250
        });
    }
    // Band 4 (lines 112..144): 1010… checkerboard (solid NTSC colour on real
    // hardware) and 1100… (a different solid colour).
    for line in 112..128 {
        poke_line(two, line, &|x| x % 2 == 0);
    }
    for line in 128..144 {
        poke_line(two, line, &|x| x % 4 < 2);
    }
    // Band 5 (lines 144..192): phase ramp — a 24-px white block whose left
    // edge shifts 1 px per line, showing every edge phase.
    for line in 144..192 {
        let x0 = 40 + (line - 144);
        poke_line(two, line, &move |x| (x0..x0 + 24).contains(&x));
    }
}

#[test]
fn render_experiment() {
    let out = std::env::var("EWM_DHGR_OUT").unwrap_or_else(|_| "/tmp".into());
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, 0xc050); // GRAPHICS
    set(&mut two, 0xc057); // HIRES
    set(&mut two, 0xc00d); // 80COL
    set(&mut two, 0xc05e); // AN3 off -> DHIRES on
    build_scene(&mut two);

    for (name, scheme, mode) in [
        ("mono", ColorScheme::Monochrome, DhgrColorMode::Aligned),
        ("aligned", ColorScheme::Color, DhgrColorMode::Aligned),
        ("sliding", ColorScheme::Color, DhgrColorMode::Sliding),
    ] {
        let mut scr = Scr::new(PixelLayout::Argb8888);
        scr.set_color_scheme(scheme);
        scr.set_dhgr_color_mode(mode);
        scr.update(&two, 0, 40);
        let bmp = encode_bmp(scr.frame(TwoType::Apple2E), SCR_WIDTH_E, SCR_HEIGHT);
        std::fs::write(format!("{out}/dhgr_{name}.bmp"), &bmp).unwrap();
    }
}
