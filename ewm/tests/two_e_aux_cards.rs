//! Swappable //e auxiliary-slot cards, driven through the bus: the
//! RamWorks III banked memory (register $C073), the plain 1K 80-Column
//! Text Card, and the default Extended 80-Column Text Card. See
//! `ewm/src/aux.rs` for the card trait.

use ewm::aux;
use ewm::two::{Two, TwoType};

const STORE80_ON: u16 = 0xc001;
const RAMRD_ON: u16 = 0xc003;
const RAMRD_OFF: u16 = 0xc002;
const RAMWRT_ON: u16 = 0xc005;
const RAMWRT_OFF: u16 = 0xc004;
const ALTZP_ON: u16 = 0xc009;
const ALTZP_OFF: u16 = 0xc008;
const PAGE2: u16 = 0xc055;
const BANK: u16 = 0xc073;

fn machine(card: &str) -> Two {
    Two::new_with_aux(TwoType::Apple2EEnhanced, Some(aux::parse(card).unwrap())).unwrap()
}

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}

// --- RamWorks III ---

#[test]
fn ramworks_banks_are_selected_at_c073_and_isolated() {
    let mut two = machine("ramworksiii:256k");
    set(&mut two, RAMWRT_ON);
    set(&mut two, RAMRD_ON);
    for bank in [0u8, 1, 2] {
        two.cpu.mem.write(BANK, bank);
        two.cpu.mem.write(0x0300, 0x10 + bank);
    }
    for bank in [0u8, 1, 2] {
        two.cpu.mem.write(BANK, bank);
        assert_eq!(
            two.cpu.mem.read(0x0300),
            0x10 + bank,
            "bank {bank} keeps its own value"
        );
    }
}

#[test]
fn ramworks_altzp_is_per_bank() {
    let mut two = machine("ramworksiii:256k");
    set(&mut two, ALTZP_ON);
    two.cpu.mem.write(BANK, 0);
    two.cpu.mem.write(0x0050, 0xa0);
    two.cpu.mem.write(BANK, 1);
    two.cpu.mem.write(0x0050, 0xa1);
    assert_eq!(two.cpu.mem.read(0x0050), 0xa1);
    two.cpu.mem.write(BANK, 0);
    assert_eq!(two.cpu.mem.read(0x0050), 0xa0, "bank 0 aux ZP intact");
    set(&mut two, ALTZP_OFF);
}

#[test]
fn ramworks_80store_pins_to_bank_0() {
    let mut two = machine("ramworksiii:256k");
    set(&mut two, STORE80_ON);
    set(&mut two, PAGE2);
    two.cpu.mem.write(BANK, 5); // CPU bank far from 0
    two.cpu.mem.write(0x0400, 0xc1); // 80STORE aux write -> bank 0
    assert_eq!(
        two.aux_ram()[0x0400],
        0xc1,
        "the display write landed in bank 0 (the renderer's view)"
    );
    // Bank 5's own $0400, read via RAMRD, is untouched.
    set(&mut two, 0xc054); // PAGE1: release the 80STORE override
    set(&mut two, RAMRD_ON);
    assert_eq!(two.cpu.mem.read(0x0400), 0x00, "bank 5 untouched");
    set(&mut two, RAMRD_OFF);
}

#[test]
fn ramworks_unpopulated_banks_float() {
    let mut two = machine("ramworksiii:256k"); // 4 banks
    set(&mut two, RAMWRT_ON);
    set(&mut two, RAMRD_ON);
    two.cpu.mem.write(BANK, 1);
    two.cpu.mem.write(0x0300, 0x42);
    two.cpu.mem.write(BANK, 9); // unpopulated
    assert_eq!(two.cpu.mem.read(0x0300), 0xff, "reads float");
    two.cpu.mem.write(0x0300, 0x99); // dropped
    two.cpu.mem.write(BANK, 1);
    assert_eq!(two.cpu.mem.read(0x0300), 0x42, "populated data intact");
}

#[test]
fn ramworks_sizing_probe_counts_the_banks() {
    let mut two = machine("ramworksiii:256k");
    set(&mut two, RAMWRT_ON);
    set(&mut two, RAMRD_ON);
    // Signature pass, then count banks that echo their signature.
    for bank in 0..16u8 {
        two.cpu.mem.write(BANK, bank);
        two.cpu.mem.write(0x0300, 0x80 | bank);
    }
    let mut populated = 0;
    for bank in 0..16u8 {
        two.cpu.mem.write(BANK, bank);
        if two.cpu.mem.read(0x0300) == 0x80 | bank {
            populated += 1;
        }
    }
    assert_eq!(populated, 4, "a 256k card is exactly 4 banks");
}

#[test]
fn ramworks_machine_still_passes_the_rom_self_test() {
    let mut two = machine("ramworksiii:1m");
    two.set_button(1, 0x80); // Solid-Apple held during reset
    two.cpu.reset();
    let mut spent = 0u64;
    while !two.text_screen().contains("System OK") {
        let mut done = 0u64;
        while done < 10_000_000 {
            done += two.cpu.step() as u64;
        }
        spent += 10_000_000;
        assert!(
            spent < 300_000_000,
            "self-test did not report OK; screen was:\n{}",
            two.text_screen()
        );
    }
}

// --- The plain 1K 80-Column Text Card ---

#[test]
fn text80_card_supports_80_column_text_only() {
    let mut two = machine("80col");
    // The aux half of text page 1 exists: 80STORE+PAGE2 writes land and the
    // 80-column scrape sees them.
    set(&mut two, STORE80_ON);
    set(&mut two, 0xc00d); // 80COL on
    set(&mut two, PAGE2);
    two.cpu.mem.write(0x0400, b'A' | 0x80); // aux -> display column 0
    set(&mut two, 0xc054); // PAGE1
    two.cpu.mem.write(0x0400, b'B' | 0x80); // main -> display column 1
    let first = two.text_screen_80();
    assert!(first.starts_with("AB"), "80-column text works: {first:?}");

    // But there is no general aux memory behind RAMRD/RAMWRT...
    set(&mut two, 0xc000); // 80STORE off
    set(&mut two, RAMWRT_ON);
    set(&mut two, RAMRD_ON);
    two.cpu.mem.write(0x0300, 0x42);
    assert_eq!(two.cpu.mem.read(0x0300), 0xff, "aux body floats");
    // ...no aux zero page...
    set(&mut two, RAMRD_OFF);
    set(&mut two, RAMWRT_OFF);
    set(&mut two, ALTZP_ON);
    two.cpu.mem.write(0x0050, 0x42);
    assert_eq!(two.cpu.mem.read(0x0050), 0xff, "aux ZP floats");
    set(&mut two, ALTZP_OFF);
    // ...and no bank register.
    two.cpu.mem.write(BANK, 3);
    set(&mut two, PAGE2);
    set(&mut two, STORE80_ON);
    assert_eq!(two.cpu.mem.read(0x0400), b'A' | 0x80, "text page unchanged");
}

// --- The default Extended 80-Column Text Card ---

#[test]
fn extended_card_ignores_the_bank_register() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap(); // default card
    set(&mut two, RAMWRT_ON);
    set(&mut two, RAMRD_ON);
    two.cpu.mem.write(0x0300, 0x42);
    two.cpu.mem.write(BANK, 7); // must change nothing
    assert_eq!(two.cpu.mem.read(0x0300), 0x42, "single fixed bank");
}

#[test]
fn ramworks_is_rejected_on_the_2plus() {
    let Err(err) = Two::new_with_aux(
        TwoType::Apple2Plus,
        Some(aux::parse("ramworksiii").unwrap()),
    ) else {
        panic!("a ][+ with an aux card must be rejected");
    };
    assert!(err.contains("auxiliary slot"), "clear error: {err}");
}
