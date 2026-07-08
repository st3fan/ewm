//! Enhanced //e double-resolution control path + double lo-res (Phase 6a).
//! IOUDIS (`$C07E`/`$C07F`) gates the dual-purpose `$C05E`/`$C05F` switches:
//! while IOUDIS is on (the reset default) they are the DHIRES switch, while it
//! is off they are annunciator 3. DLGR is 80-column lo-res (aux even / main
//! odd, 7 px each → 560), enabled by DHIRES + 80COL + lo-res graphics.

use ewm::scr::{PixelLayout, SCR_HEIGHT, SCR_WIDTH_E, Scr, encode_bmp};
use ewm::two::{Two, TwoType};

const GRAPHICS: u16 = 0xc050;
const LORES: u16 = 0xc056;
const COL80_ON: u16 = 0xc00d;
const RAMWRT_OFF: u16 = 0xc004;
const RAMWRT_ON: u16 = 0xc005;
const DHIRES_ON: u16 = 0xc05e; // under IOUDIS
const DHIRES_OFF: u16 = 0xc05f;
const SET_IOUDIS: u16 = 0xc07e;
const CLR_IOUDIS: u16 = 0xc07f;
const RDIOUDIS: u16 = 0xc07e;
const RDDHIRES: u16 = 0xc07f;

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}
fn bit7(two: &mut Two, addr: u16) -> u8 {
    two.cpu.mem.read(addr) & 0x80
}

#[test]
fn ioudis_defaults_on_and_toggles() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    assert_eq!(bit7(&mut two, RDIOUDIS), 0x80, "IOUDIS on at reset");
    set(&mut two, CLR_IOUDIS);
    assert_eq!(bit7(&mut two, RDIOUDIS), 0x00, "CLRIOUDIS turns it off");
    set(&mut two, SET_IOUDIS);
    assert_eq!(bit7(&mut two, RDIOUDIS), 0x80, "SETIOUDIS turns it on");
}

#[test]
fn dhires_switch_under_ioudis() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    assert_eq!(bit7(&mut two, RDDHIRES), 0x00, "DHIRES off at reset");
    // IOUDIS is on by default, so $C05E/$C05F are the DHIRES switch.
    set(&mut two, DHIRES_ON);
    assert_eq!(bit7(&mut two, RDDHIRES), 0x80, "$C05E turns DHIRES on");
    set(&mut two, DHIRES_OFF);
    assert_eq!(bit7(&mut two, RDDHIRES), 0x00, "$C05F turns DHIRES off");
    // Reads of $C05E/$C05F toggle the switch too.
    two.cpu.mem.read(DHIRES_ON);
    assert_eq!(
        bit7(&mut two, RDDHIRES),
        0x80,
        "reading $C05E also sets DHIRES"
    );
}

#[test]
fn an3_takes_over_when_ioudis_off() {
    // With IOUDIS off, $C05E/$C05F control AN3 and must NOT change DHIRES.
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, DHIRES_ON); // DHIRES on (IOUDIS still on)
    assert_eq!(bit7(&mut two, RDDHIRES), 0x80);
    set(&mut two, CLR_IOUDIS); // now $C05E/$C05F are AN3
    set(&mut two, DHIRES_OFF); // would clear DHIRES if it still controlled it
    assert_eq!(
        bit7(&mut two, RDDHIRES),
        0x80,
        "DHIRES unchanged while IOUDIS off routes $C05E/$C05F to AN3"
    );
}

/// Write `byte` to double lo-res display column `col` of `row`: aux for even
/// columns, main for odd (RAMWRT selects the bank; 80STORE is off).
fn put_dlgr(two: &mut Two, row: usize, col: usize, byte: u8) {
    let base = (0x400 + 0x80 * (row % 8) + 0x28 * (row / 8)) as u16;
    set(
        two,
        if col.is_multiple_of(2) {
            RAMWRT_ON
        } else {
            RAMWRT_OFF
        },
    );
    two.cpu.mem.write(base + (col / 2) as u16, byte);
    set(two, RAMWRT_OFF);
}

fn enable_dlgr(two: &mut Two) {
    set(two, GRAPHICS);
    set(two, LORES);
    set(two, COL80_ON);
    set(two, DHIRES_ON); // IOUDIS on by default, so this is DHIRES
}

#[test]
fn dlgr_screen_matches_golden_bmp() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    enable_dlgr(&mut two);
    // 80 vertical colour bars: display column c is a solid block of colour
    // c % 16 (both nibbles), laid down through the aux/main interleave.
    for row in 0..24 {
        for col in 0..80 {
            let color = (col % 16) as u8;
            put_dlgr(&mut two, row, col, color | (color << 4));
        }
    }

    let mut scr = Scr::new(PixelLayout::Argb8888);
    scr.set_color_scheme(ewm::scr::ColorScheme::Color);
    scr.update(&two, 0, 40);
    let bmp = encode_bmp(scr.frame(TwoType::Apple2E), SCR_WIDTH_E, SCR_HEIGHT);

    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/two-e-dlgr.bmp");
    if std::env::var("EWM_WRITE_GOLDEN").is_ok() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/golden")).unwrap();
        std::fs::write(golden_path, &bmp).unwrap();
        return;
    }
    match std::fs::read(golden_path) {
        Ok(golden) => assert_eq!(bmp, golden, "DLGR screen differs from the golden BMP"),
        Err(_) => panic!(
            "golden BMP missing — generate it with:\n  \
             EWM_WRITE_GOLDEN=1 cargo test -p ewm dlgr_screen_matches_golden_bmp"
        ),
    }
}
