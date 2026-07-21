//! Enhanced //e text decode (Phase 3a): the ALTCHARSET soft switch (`$C01E`)
//! selects the primary vs alternate glyph set, so the //e renders lower case
//! and MouseText. Headless — checks glyph selection (via the `ChrE` tables)
//! and the //e-aware `text_screen` scrape. Pixel rendering is Phase 5.

use ewm::chr::{CharSet, ChrE};
use ewm::two::{Two, TwoType};

fn poke(two: &mut Two, addr: u16, byte: u8) {
    two.cpu.mem.write(addr, byte); // $0400 text page is base RAM
}

#[test]
fn altcharset_selects_the_mousetext_glyph() {
    let chre = ChrE::new();
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();

    // ALTCHARSET on: $56 in the text page is the MouseText checkerboard.
    two.cpu.mem.write(0xc00f, 0); // SETALTCHARSET
    assert!(two.alt_charset());
    poke(&mut two, 0x0400, 0x56);
    let code = two.ram()[0x0400];
    assert_eq!(
        chre.glyph(two.alt_charset(), code),
        chre.bitmap(CharSet::Alternate, 0x56),
        "ALTCHARSET on selects the alternate (MouseText) glyph"
    );
    assert_ne!(
        chre.glyph(two.alt_charset(), code),
        chre.bitmap(CharSet::Primary, 0x56),
        "and it is not the primary-set glyph"
    );

    // ALTCHARSET off: the same byte selects the primary-set glyph.
    two.cpu.mem.write(0xc00e, 0); // CLRALTCHARSET
    assert!(!two.alt_charset());
    assert_eq!(
        chre.glyph(two.alt_charset(), code),
        chre.bitmap(CharSet::Primary, 0x56),
    );
}

#[test]
fn text_screen_preserves_lower_case() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    two.cpu.mem.write(0xc00f, 0); // ALTCHARSET on
    // Normal lower-case screen codes for "abc" ($E1 $E2 $E3).
    poke(&mut two, 0x0400, 0xe1);
    poke(&mut two, 0x0401, 0xe2);
    poke(&mut two, 0x0402, 0xe3);
    let first = two.text_screen().lines().next().unwrap().to_string();
    assert!(
        first.starts_with("abc"),
        "expected lower case; got {first:?}"
    );
}

#[test]
fn altcharset_changes_the_inverse_lower_case_range() {
    // $61 is inverse lower-case 'a' in the alternate set, but flashing '!'
    // ($61 & $3F = $21) in the primary set — so the scrape depends on
    // ALTCHARSET for the $60-$7F range.
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    poke(&mut two, 0x0400, 0x61);

    two.cpu.mem.write(0xc00f, 0); // ALTCHARSET on
    assert_eq!(
        two.text_screen().chars().next().unwrap(),
        'a',
        "alternate set decodes $61 as lower-case 'a'"
    );

    two.cpu.mem.write(0xc00e, 0); // ALTCHARSET off
    assert_eq!(
        two.text_screen().chars().next().unwrap(),
        '!',
        "primary set decodes $61 as '!'"
    );
}
