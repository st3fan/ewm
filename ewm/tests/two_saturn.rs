//! The Saturn 128K RAM Board in slot 0: bank switching on the bus, and
//! the Language Card compatibility story end to end — DOS 3.3 boots
//! treating the board as a 16K card, loads Integer BASIC into bank 1,
//! and INT/FP swap between the ROM and card BASICs.

use std::collections::BTreeMap;

use ewm::two::{Slot0, SlotDevice, Two, TwoType};

const DOS33: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../disks/DOS33-SystemMaster.dsk"
);

fn saturn_machine(slots: &[(u8, SlotDevice)]) -> Two {
    let slots: BTreeMap<u8, SlotDevice> = slots.iter().copied().collect();
    Two::new_with_slots(TwoType::Apple2Plus, None, Slot0::Saturn128, &slots)
        .expect("must construct")
}

/// Step until the predicate holds, with a cycle cap; checked every ~50K
/// cycles (the two_slots.rs pattern).
fn step_until(two: &mut Two, cap: u64, what: &str, pred: impl Fn(&Two) -> bool) {
    let mut cycles = 0u64;
    while !pred(two) {
        assert!(
            cycles < cap,
            "gave up waiting for {what} after {cycles} cycles; screen was:\n{}",
            two.text_screen()
        );
        let target = cycles + 50_000;
        while cycles < target {
            cycles += two.cpu.step() as u64;
        }
    }
}

/// Type a line through the keyboard latch, waiting for the strobe to be
/// consumed after each key.
fn type_line(two: &mut Two, line: &str) {
    for &b in line.as_bytes() {
        two.key(b);
        step_until(two, 2_000_000, "key strobe", |two| {
            two.key_register() & 0x80 == 0
        });
    }
    two.key(0x0d);
    step_until(two, 2_000_000, "return strobe", |two| {
        two.key_register() & 0x80 == 0
    });
}

#[test]
fn eight_banks_hold_independent_contents_on_the_bus() {
    let mut two = saturn_machine(&[]);
    let mem = &mut two.cpu.mem;

    // Power-up: ROM at $D000-$FFFF, bank 1, write-protected.
    let reset_lo = mem.read(0xfffc);
    mem.write(0xd000, 0x21);
    assert_ne!(mem.read(0xd000), 0x21, "power-up state must be ROM");

    // Write-enable RAM ($C083 twice) and stamp every 16K bank.
    mem.read(0xc083);
    mem.read(0xc083);
    for bank in 0..8u16 {
        let offset = if bank < 4 {
            0xc084 + bank
        } else {
            0xc088 + bank
        };
        mem.read(offset);
        mem.write(0xd000, 0x10 + bank as u8);
        mem.write(0xe000, 0x20 + bank as u8);
        mem.write(0xffff, 0x30 + bank as u8);
    }
    for bank in 0..8u16 {
        let offset = if bank < 4 {
            0xc084 + bank
        } else {
            0xc088 + bank
        };
        mem.read(offset);
        assert_eq!(mem.read(0xd000), 0x10 + bank as u8, "bank {bank} $D000");
        assert_eq!(mem.read(0xe000), 0x20 + bank as u8, "bank {bank} $E000");
        assert_eq!(mem.read(0xffff), 0x30 + bank as u8, "bank {bank} $FFFF");
    }

    // The B 4K bank splits $D000 but shares $E000-$FFFF within the bank.
    mem.read(0xc08b); // RAM read/write, 4K bank B (state access, bank 8 kept)
    mem.read(0xc08b);
    assert_ne!(mem.read(0xd000), 0x17, "bank B is not bank A");
    mem.write(0xd000, 0x99);
    assert_eq!(mem.read(0xe000), 0x27, "$E000 stays per-16K-bank");
    mem.read(0xc083); // back to 4K bank A
    assert_eq!(mem.read(0xd000), 0x17);

    // ROM fall-through still works after all that.
    mem.read(0xc081);
    assert_eq!(mem.read(0xfffc), reset_lo, "ROM must return on $C081");
}

#[test]
fn dos33_treats_the_saturn_as_a_language_card() {
    // DOS 3.3's HELLO finds a "16K card" in slot 0 and loads Integer
    // BASIC into bank 1; INT switches to it (the > prompt) and FP back
    // (the ] prompt). This is the Saturn's whole compatibility claim.
    let mut two = saturn_machine(&[(6, SlotDevice::DiskII)]);
    two.load_disk_at(6, 0, DOS33).expect("load slot 6");
    two.cpu.reset();
    step_until(&mut two, 400_000_000, "the ] prompt", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });

    type_line(&mut two, "INT");
    step_until(&mut two, 50_000_000, "the Integer BASIC > prompt", |two| {
        two.text_screen()
            .lines()
            .any(|l| l.trim_start().starts_with('>'))
    });

    type_line(&mut two, "FP");
    step_until(&mut two, 50_000_000, "the Applesoft ] prompt", |two| {
        let text = two.text_screen();
        text.lines()
            .rfind(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim() == "]")
    });
}
