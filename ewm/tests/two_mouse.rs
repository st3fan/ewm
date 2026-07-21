//! The AppleMouse II card, driven through its real firmware (plans/20260721-01
//! M2). A ][+ with a mouse in slot 4 runs 6502 code that calls the documented
//! entry points — InitMouse, ClampMouse, SetMouse, PosMouse, ReadMouse — found
//! through the `$Cn12` offset table, and the result lands in the slot's screen
//! holes, clamped. Mirrors `two_clk.rs`'s "drive the firmware, read the holes"
//! shape.

use std::collections::BTreeMap;

use ewm::two::{Slot0, SlotDevice, Two, TwoType};

/// A ][+ (48K) whose only card is a mouse in slot 4.
fn machine_with_mouse() -> Two {
    Two::new_with_slots(
        TwoType::Apple2Plus,
        None,
        Slot0::Empty,
        &BTreeMap::from([(4, SlotDevice::Mouse)]),
    )
    .expect("a ][+ with a slot-4 mouse must construct")
}

fn read(two: &mut Two, addr: u16) -> u8 {
    two.cpu.mem.read(addr)
}

/// The low byte of routine `k` (SetMouse=0 … InitMouse=7) from the `$C412`
/// offset table, formed into its full `$C4xx` entry address.
fn entry(two: &mut Two, k: u16) -> u16 {
    0xc400 | read(two, 0xc412 + k) as u16
}

#[test]
fn card_is_identifiable_by_its_firmware_bytes() {
    let mut two = machine_with_mouse();
    // The Autostart scan must not see a Disk II; ProDOS/software see the
    // Pascal 1.1 + AppleMouse identification.
    assert_ne!(read(&mut two, 0xc401), 0x20, "not a Disk II boot signature");
    assert_eq!(read(&mut two, 0xc405), 0x38);
    assert_eq!(read(&mut two, 0xc407), 0x18);
    assert_eq!(read(&mut two, 0xc40b), 0x01);
    assert_eq!(read(&mut two, 0xc40c), 0x20);
    assert_eq!(read(&mut two, 0xc4fb), 0xd6, "AppleMouse ID");
}

#[test]
fn init_clamp_pos_read_through_the_firmware_deposits_clamped_holes() {
    let mut two = machine_with_mouse();

    let set_mouse = entry(&mut two, 0);
    let read_mouse = entry(&mut two, 2);
    let pos_mouse = entry(&mut two, 4);
    let clamp_mouse = entry(&mut two, 5);
    let init_mouse = entry(&mut two, 7);

    // Assemble a program at $0300 that exercises the flow. The slot-4 screen
    // holes: Xlo/Xhi $047C/$04FC, Ylo/Yhi $057C/$05FC, status $077C, mode
    // $07FC. ClampMouse takes min in the X holes, max in the Y holes.
    let mut p: Vec<u8> = Vec::new();
    let lda_sta = |p: &mut Vec<u8>, imm: u8, addr: u16| {
        p.extend([0xa9, imm, 0x8d, addr as u8, (addr >> 8) as u8]);
    };
    let jsr = |p: &mut Vec<u8>, addr: u16| p.extend([0x20, addr as u8, (addr >> 8) as u8]);
    let lda_jsr = |p: &mut Vec<u8>, imm: u8, addr: u16| {
        p.extend([0xa9, imm]);
        p.extend([0x20, addr as u8, (addr >> 8) as u8]);
    };

    jsr(&mut p, init_mouse); // clamp 0..=1023, mouse off
    // ClampX: min = 100 ($0064), max = 700 ($02BC), A = 0.
    lda_sta(&mut p, 0x64, 0x047c);
    lda_sta(&mut p, 0x00, 0x04fc);
    lda_sta(&mut p, 0xbc, 0x057c);
    lda_sta(&mut p, 0x02, 0x05fc);
    lda_jsr(&mut p, 0x00, clamp_mouse);
    // ClampY: min = 200 ($00C8), max = 500 ($01F4), A = 1.
    lda_sta(&mut p, 0xc8, 0x047c);
    lda_sta(&mut p, 0x00, 0x04fc);
    lda_sta(&mut p, 0xf4, 0x057c);
    lda_sta(&mut p, 0x01, 0x05fc);
    lda_jsr(&mut p, 0x01, clamp_mouse);
    // SetMouse: mode = 1 (mouse on).
    lda_jsr(&mut p, 0x01, set_mouse);
    // PosMouse to (9999, 50): X far past maxX, Y below minY.
    lda_sta(&mut p, 0x0f, 0x047c); // 9999 = $270F
    lda_sta(&mut p, 0x27, 0x04fc);
    lda_sta(&mut p, 0x32, 0x057c); // 50 = $0032
    lda_sta(&mut p, 0x00, 0x05fc);
    jsr(&mut p, pos_mouse);
    // ReadMouse: deposit clamped X/Y/status/mode into the holes.
    jsr(&mut p, read_mouse);
    // Park.
    let park = 0x0300 + p.len() as u16;
    p.extend([0x4c, park as u8, (park >> 8) as u8]); // JMP self

    // Load and run.
    for (i, &b) in p.iter().enumerate() {
        two.cpu.mem.write(0x0300 + i as u16, b);
    }
    two.cpu.reset();
    two.cpu.pc = 0x0300;
    for _ in 0..200_000 {
        two.cpu.step();
        if two.cpu.pc == park {
            break;
        }
    }
    assert_eq!(
        two.cpu.pc, park,
        "the program should reach its parking loop"
    );

    // X clamped to maxX = 700 ($02BC), Y clamped to minY = 200 ($00C8).
    assert_eq!(read(&mut two, 0x047c), 0xbc, "X low (700)");
    assert_eq!(read(&mut two, 0x04fc), 0x02, "X high (700)");
    assert_eq!(read(&mut two, 0x057c), 0xc8, "Y low (200)");
    assert_eq!(read(&mut two, 0x05fc), 0x00, "Y high (200)");
    // No host button/movement yet.
    assert_eq!(
        read(&mut two, 0x077c),
        0x00,
        "status: no button, no movement"
    );
    // Mode reads back what SetMouse wrote.
    assert_eq!(read(&mut two, 0x07fc), 0x01, "mode");
}
