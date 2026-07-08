//! Enhanced //e ALTZP + language-card aux banking (Phase 4b). ALTZP
//! (`$C008`/`$C009`) selects main vs aux for both the zero-page/stack region
//! (`$0000-$01FF`) and the built-in language-card RAM (`$D000-$FFFF`), which
//! on the //e lives in the memory-management unit (`IouE`), not the `Alc`
//! peripheral card. Headless — driven through the memory bus.

use ewm::two::{Two, TwoType};

const RAMWRT_OFF: u16 = 0xc004;
const ALTZP_OFF: u16 = 0xc008;
const ALTZP_ON: u16 = 0xc009;

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0); // write-to-set soft switch
}

#[test]
fn zero_page_and_stack_follow_altzp() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();

    // ALTZP off: zero page and the stack live in main.
    set(&mut two, ALTZP_OFF);
    two.cpu.mem.write(0x0050, 0x11);
    two.cpu.mem.write(0x01ff, 0x22);
    assert_eq!(two.ram()[0x0050], 0x11);
    assert_eq!(two.ram()[0x01ff], 0x22);

    // ALTZP on: they live in aux (main is untouched).
    set(&mut two, ALTZP_ON);
    two.cpu.mem.write(0x0050, 0x33);
    two.cpu.mem.write(0x01ff, 0x44);
    assert_eq!(two.aux_ram()[0x0050], 0x33);
    assert_eq!(two.aux_ram()[0x01ff], 0x44);
    assert_eq!(two.ram()[0x0050], 0x11, "main ZP untouched by aux write");

    // Reads follow ALTZP too.
    assert_eq!(two.cpu.mem.read(0x0050), 0x33);
    set(&mut two, ALTZP_OFF);
    assert_eq!(two.cpu.mem.read(0x0050), 0x11);

    // RDALTZP ($C016) reflects state.
    assert_eq!(two.cpu.mem.read(0xc016) & 0x80, 0x00);
    set(&mut two, ALTZP_ON);
    assert_eq!(two.cpu.mem.read(0xc016) & 0x80, 0x80);
}

#[test]
fn altzp_does_not_affect_the_main_body() {
    // $0200-$BFFF still follows RAMRD/RAMWRT, not ALTZP.
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, ALTZP_ON); // ALTZP on
    set(&mut two, RAMWRT_OFF); // but RAMWRT off
    two.cpu.mem.write(0x0300, 0x77);
    assert_eq!(two.ram()[0x0300], 0x77, "main body ignores ALTZP");
    assert_eq!(two.aux_ram()[0x0300], 0x00);
}

/// Enable the language card's RAM for reading and writing, bank 1: two reads
/// of `$C08B` (bit 3 set = bank 1; low bits = read-enable + write-enable).
fn enable_lc_bank1_rw(two: &mut Two) {
    two.cpu.mem.read(0xc08b);
    two.cpu.mem.read(0xc08b);
}

#[test]
fn language_card_ram_has_main_and_aux_banks() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    enable_lc_bank1_rw(&mut two);

    // ALTZP off -> the write lands in the main LC bank.
    set(&mut two, ALTZP_OFF);
    two.cpu.mem.write(0xd000, 0xaa);
    // ALTZP on -> a different value lands in the aux LC bank.
    set(&mut two, ALTZP_ON);
    two.cpu.mem.write(0xd000, 0xbb);

    // Each bank keeps its own value.
    assert_eq!(two.cpu.mem.read(0xd000), 0xbb, "aux LC bank");
    set(&mut two, ALTZP_OFF);
    assert_eq!(two.cpu.mem.read(0xd000), 0xaa, "main LC bank");
}

#[test]
fn language_card_falls_through_to_rom_when_not_read_enabled() {
    // A fresh card is inactive: $D000-$FFFF reads the banked ROM, and $FFFC
    // is the //e reset vector ($FA62).
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    let reset = (two.cpu.mem.read(0xfffc) as u16) | ((two.cpu.mem.read(0xfffd) as u16) << 8);
    assert_eq!(reset, 0xfa62, "reset vector reads from the LC ROM");
}

#[test]
fn rdlcbnk2_rdlcram_report_state() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    // Fresh: reading ROM, RDLCRAM ($C012) low.
    assert_eq!(two.cpu.mem.read(0xc012) & 0x80, 0x00, "RDLCRAM: ROM");

    enable_lc_bank1_rw(&mut two);
    assert_eq!(
        two.cpu.mem.read(0xc012) & 0x80,
        0x80,
        "RDLCRAM: RAM read-enabled"
    );
    assert_eq!(two.cpu.mem.read(0xc011) & 0x80, 0x00, "RDLCBNK2: bank 1");

    // Select bank 2 ($C083, bit 3 clear).
    two.cpu.mem.read(0xc083);
    assert_eq!(two.cpu.mem.read(0xc011) & 0x80, 0x80, "RDLCBNK2: bank 2");
}
