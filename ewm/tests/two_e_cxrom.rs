//! Enhanced //e `$C100-$CFFF` ROM arbitration (Phase 2b): INTCXROM / SLOTC3ROM
//! select internal firmware vs peripheral-slot ROM, with the `$C800` expansion
//! latch and its `$CFFF` reset. Headless — the switches are driven through the
//! memory bus, exactly as software does. A fresh machine powers up in slot mode
//! (INTCXROM off, SLOTC3ROM off); `reset()` does not touch these switches, so
//! the tests below that don't step have deterministic state.

use ewm::two::{Two, TwoType};

fn machine() -> Two {
    Two::new(TwoType::Apple2E).expect("apple2e must construct")
}

#[test]
fn default_slot_mode_selects_internal_c3_and_slot_cards() {
    let mut two = machine();
    // INTCXROM off, SLOTC3ROM off: $C300 is the internal 80-column firmware,
    // identified by its Pascal-1.1 protocol signature ($C305=$38, $C307=$18).
    assert_eq!(two.cpu.mem.read(0xc305), 0x38);
    assert_eq!(two.cpu.mem.read(0xc307), 0x18);
    // Peripheral-slot ROMs are visible: slot 6 Disk II ($C600=$A2) and slot 1
    // Thunderclock ($C100=$08).
    assert_eq!(two.cpu.mem.read(0xc600), 0xa2);
    assert_eq!(two.cpu.mem.read(0xc100), 0x08);
}

#[test]
fn intcxrom_selects_internal_rom_everywhere() {
    let mut two = machine();
    two.cpu.mem.write(0xc007, 0); // SETINTCXROM
    assert_eq!(
        two.cpu.mem.read(0xc015) & 0x80,
        0x80,
        "RDCXROM ($C015) reports INTCXROM"
    );
    // $C600/$C100 now read internal ROM ($8D / $4C), not the slot ROMs.
    assert_eq!(two.cpu.mem.read(0xc600), 0x8d);
    assert_eq!(two.cpu.mem.read(0xc100), 0x4c);
    // $C300 is still the internal firmware.
    assert_eq!(two.cpu.mem.read(0xc305), 0x38);

    two.cpu.mem.write(0xc006, 0); // CLRINTCXROM -> back to slot mode
    assert_eq!(two.cpu.mem.read(0xc015) & 0x80, 0x00);
    assert_eq!(two.cpu.mem.read(0xc600), 0xa2);
    assert_eq!(two.cpu.mem.read(0xc100), 0x08);
}

#[test]
fn slotc3rom_switches_c300_to_the_absent_slot3_card() {
    let mut two = machine();
    two.cpu.mem.write(0xc00b, 0); // SETSLOTC3ROM
    assert_eq!(
        two.cpu.mem.read(0xc017) & 0x80,
        0x80,
        "RDC3ROM ($C017) reports SLOTC3ROM"
    );
    // There is no slot-3 card in EWM, so $C300 reads open bus.
    assert_eq!(two.cpu.mem.read(0xc305), 0x00);

    two.cpu.mem.write(0xc00a, 0); // CLRSLOTC3ROM -> internal 80-column firmware
    assert_eq!(two.cpu.mem.read(0xc017) & 0x80, 0x00);
    assert_eq!(two.cpu.mem.read(0xc305), 0x38);
}

#[test]
fn c800_expansion_latch_and_cfff_reset() {
    let mut two = machine();
    // Fresh: the internal $C800 expansion ROM is not yet exposed -> open bus.
    assert_eq!(two.cpu.mem.read(0xc800), 0x00);
    // Touching the internal $C3xx firmware exposes the internal $C800 ROM.
    let _ = two.cpu.mem.read(0xc300);
    assert_eq!(
        two.cpu.mem.read(0xc800),
        0x4c,
        "internal $C800 is exposed after a $C3xx access"
    );
    // Reading $CFFF resets the expansion-ROM latch.
    let _ = two.cpu.mem.read(0xcfff);
    assert_eq!(
        two.cpu.mem.read(0xc800),
        0x00,
        "$CFFF resets the expansion ROM"
    );
    // Under INTCXROM, $C800-$CFFF is internal regardless of the latch.
    two.cpu.mem.write(0xc007, 0);
    assert_eq!(two.cpu.mem.read(0xc800), 0x4c);
}

#[test]
fn boots_past_the_c300_firmware_into_disk_boot() {
    // Regression on Phase 2a: with the internal $CX ROM now mapped, the //e no
    // longer gets stuck in unmapped $C3xx space. It runs the monitor and the
    // internal 80-column firmware and reaches the slot-6 Disk II boot ROM
    // ($C6xx), where — with no disk inserted — it spins waiting for a drive.
    // Booting all the way to `]` is Phase 2c.
    let mut two = machine();
    two.cpu.reset();
    for _ in 0..2_000_000 {
        two.cpu.step();
    }
    let pc = two.cpu.pc;
    assert!(
        (0xc600..=0xc6ff).contains(&pc),
        "expected to reach the Disk II boot ROM ($C6xx); pc=${pc:04X}"
    );
}
