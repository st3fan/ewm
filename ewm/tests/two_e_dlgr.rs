//! Enhanced //e double-resolution control path + double lo-res (Phase 6a,
//! corrected). On the //e, `$C05E`/`$C05F` drive DHIRES directly through
//! annunciator 3 — `$C05E` (AN3 off) turns double-res on, `$C05F` (AN3 on) off,
//! on any read or write. There is no IOUDIS gate (that is a //c switch the //e
//! Tech Ref documents in error — verified floating on real //e hardware; see
//! the AN3/DHIRES note in the plan doc). DLGR is 80-column lo-res (aux even /
//! main odd, 7 px each → 560), enabled by DHIRES + 80COL + lo-res graphics.

use ewm::scr::{PixelLayout, SCR_HEIGHT, SCR_WIDTH_E, Scr, encode_bmp};
use ewm::two::{Two, TwoType};

const GRAPHICS: u16 = 0xc050;
const LORES: u16 = 0xc056;
const COL80_ON: u16 = 0xc00d;
const RAMWRT_OFF: u16 = 0xc004;
const RAMWRT_ON: u16 = 0xc005;
const DHIRES_ON: u16 = 0xc05e; // AN3 off
const DHIRES_OFF: u16 = 0xc05f; // AN3 on

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}

#[test]
fn an3_drives_dhires() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    assert!(!two.dhires(), "DHIRES off at reset (AN3 on)");
    set(&mut two, DHIRES_ON);
    assert!(two.dhires(), "$C05E (AN3 off) turns DHIRES on");
    set(&mut two, DHIRES_OFF);
    assert!(!two.dhires(), "$C05F (AN3 on) turns DHIRES off");
    // Reads act on the switch as well as writes.
    two.cpu.mem.read(DHIRES_ON);
    assert!(two.dhires(), "reading $C05E turns DHIRES on");
    two.cpu.mem.read(DHIRES_OFF);
    assert!(!two.dhires(), "reading $C05F turns DHIRES off");
}

#[test]
fn ioudis_addresses_are_inert() {
    // $C07E/$C07F (IOUDIS) do not exist on the //e — they must not affect
    // DHIRES, and $C05E/$C05F control it unconditionally.
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, DHIRES_ON);
    set(&mut two, 0xc07f); // phantom CLRIOUDIS
    set(&mut two, 0xc07e); // phantom SETIOUDIS
    two.cpu.mem.read(0xc07e); // phantom RDIOUDIS
    two.cpu.mem.read(0xc07f); // phantom RDDHIRES
    assert!(two.dhires(), "$C07E/$C07F are inert; DHIRES unchanged");
    set(&mut two, DHIRES_OFF);
    assert!(
        !two.dhires(),
        "$C05F still clears DHIRES regardless of $C07x"
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
    set(two, DHIRES_ON); // $C05E: AN3 off -> DHIRES on
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
