//! The AppleMouse II card (plans/20260721-03: the real 6520 PIA + 6805 + the
//! `342-0270-C` ROM), driven through its **real ROM firmware**. A ][+ with a
//! mouse in slot 4 runs 6502 code that calls the documented entry points
//! (found through the `$Cn12` offset table) with the Apple II slot-firmware
//! register convention (X = `$Cn`, Y = slot×16); the ROM's `$xx70`
//! page-switching banks its own `$Cn00` ROM mid-routine, drives the PIA
//! handshake, and the result lands in the slot's screen holes.
//!
//! The VBL-interrupt test (P4) drives the real firmware's interrupt path: the
//! 6805 asserts the slot IRQ line on VBL, the ROM's user IRQ vector routes to
//! a handler that calls the real ServeMouse, and ServeMouse reports the source
//! and de-asserts the line.

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

/// Set X=$Cn / Y=$n0 before a JSR — the Apple II slot-firmware register
/// convention the real ROM's routines require (X = the `$Cn` ROM page for the
/// screen-hole index, Y = slot×16 for the `$C0nX` DEVSEL). Slot 4.
fn ldxy_jsr(p: &mut Vec<u8>, addr: u16) {
    p.extend([0xa2, 0xc4, 0xa0, 0x40]); // LDX #$C4; LDY #$40
    p.extend([0x20, addr as u8, (addr >> 8) as u8]);
}
fn lda_ldxy_jsr(p: &mut Vec<u8>, imm: u8, addr: u16) {
    p.extend([0xa9, imm]); // LDA #imm
    ldxy_jsr(p, addr);
}
fn lda_sta(p: &mut Vec<u8>, imm: u8, addr: u16) {
    p.extend([0xa9, imm, 0x8d, addr as u8, (addr >> 8) as u8]);
}

#[test]
fn init_clamp_pos_read_through_the_firmware_deposits_clamped_holes() {
    let mut two = machine_with_mouse();

    let set_mouse = entry(&mut two, 0);
    let read_mouse = entry(&mut two, 2);
    let pos_mouse = entry(&mut two, 4);
    let clamp_mouse = entry(&mut two, 5);
    let init_mouse = entry(&mut two, 7);

    // Screen holes. Pos/Read use the slot-4 holes — X = ($047C lo, $057C hi),
    // Y = ($04FC lo, $05FC hi), status $077C, mode $07FC. ClampMouse instead
    // reads the FIXED slot-0 holes (the documented clamp quirk, Apple II
    // Technical Note Mouse #7): min = ($0478 lo, $0578 hi), max = ($04F8 lo,
    // $05F8 hi).
    let mut p: Vec<u8> = Vec::new();
    ldxy_jsr(&mut p, init_mouse); // clamp 0..=1023, mouse off
    // ClampX: min = 100 ($0064), max = 700 ($02BC), A = 0.
    lda_sta(&mut p, 0x64, 0x0478);
    lda_sta(&mut p, 0x00, 0x0578);
    lda_sta(&mut p, 0xbc, 0x04f8);
    lda_sta(&mut p, 0x02, 0x05f8);
    lda_ldxy_jsr(&mut p, 0x00, clamp_mouse);
    // ClampY: min = 200 ($00C8), max = 500 ($01F4), A = 1.
    lda_sta(&mut p, 0xc8, 0x0478);
    lda_sta(&mut p, 0x00, 0x0578);
    lda_sta(&mut p, 0xf4, 0x04f8);
    lda_sta(&mut p, 0x01, 0x05f8);
    lda_ldxy_jsr(&mut p, 0x01, clamp_mouse);
    // SetMouse: mode = 1 (mouse on).
    lda_ldxy_jsr(&mut p, 0x01, set_mouse);
    // PosMouse to (9999, 50): X far past maxX, Y below minY.
    lda_sta(&mut p, 0x0f, 0x047c); // Xlo (9999 = $270F)
    lda_sta(&mut p, 0x27, 0x057c); // Xhi
    lda_sta(&mut p, 0x32, 0x04fc); // Ylo (50 = $0032)
    lda_sta(&mut p, 0x00, 0x05fc); // Yhi
    ldxy_jsr(&mut p, pos_mouse);
    // ReadMouse: deposit clamped X/Y/status/mode into the slot-4 holes.
    ldxy_jsr(&mut p, read_mouse);
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
    assert_eq!(read(&mut two, 0x057c), 0x02, "X high (700)");
    assert_eq!(read(&mut two, 0x04fc), 0xc8, "Y low (200)");
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

/// The mouse's current position, read from the device (the 6805 state).
fn device_pos(two: &mut Two) -> (i16, i16) {
    two.mouse_position().unwrap()
}

#[test]
fn vbl_interrupts_fire_once_per_frame_and_serve_reports_the_source() {
    // The interrupt flagship through the REAL firmware (plans/20260721-03 P4):
    // enable VBL interrupts, install a handler through the ROM's user IRQ
    // vector ($03FE), tick N frames, and confirm the handler fired exactly
    // once per frame with the real ServeMouse reporting the VBL source (and
    // de-asserting the line so it is not re-taken) — and that a fed movement
    // reaches the position the firmware reads.
    let mut two = machine_with_mouse();
    let serve_mouse = entry(&mut two, 1);
    let set_mouse = entry(&mut two, 0);
    let init_mouse = entry(&mut two, 7);

    // Handler at $0300: JSR ServeMouse (X=$Cn, Y=$n0); read the source it
    // deposits in the status hole $077C; bump the counter; restore A (the ROM
    // IRQ handler saved it in $45); RTI.
    let mut handler: Vec<u8> = Vec::new();
    handler.extend([0xa2, 0xc4, 0xa0, 0x40]); // LDX #$C4; LDY #$40
    handler.extend([0x20, serve_mouse as u8, (serve_mouse >> 8) as u8]); // JSR ServeMouse
    handler.extend([0xad, 0x7c, 0x07]); // LDA $077C (status hole)
    handler.extend([0x8d, 0x81, 0x02]); // STA $0281 (source byte)
    handler.extend([0xee, 0x80, 0x02]); // INC $0280 (fire count)
    handler.extend([0xa5, 0x45]); // LDA $45 (the ROM handler saved A here)
    handler.push(0x40); // RTI
    for (i, &b) in handler.iter().enumerate() {
        two.cpu.mem.write(0x0300 + i as u16, b);
    }

    // Setup at $0320: point the user IRQ vector at $0300, InitMouse, SetMouse
    // mode $09 (mouse on + VBL interrupt), zero the counters, CLI, spin. Each
    // firmware call takes X=$Cn, Y=$n0.
    let mut s: Vec<u8> = Vec::new();
    let lda_sta = |s: &mut Vec<u8>, imm: u8, addr: u16| {
        s.extend([0xa9, imm, 0x8d, addr as u8, (addr >> 8) as u8]);
    };
    lda_sta(&mut s, 0x00, 0x03fe);
    lda_sta(&mut s, 0x03, 0x03ff);
    ldxy_jsr(&mut s, init_mouse); // InitMouse
    lda_ldxy_jsr(&mut s, 0x09, set_mouse); // SetMouse mode $09
    lda_sta(&mut s, 0x00, 0x0280);
    lda_sta(&mut s, 0x00, 0x0281);
    s.push(0x58); // CLI
    let spin = 0x0320 + s.len() as u16;
    s.extend([0x4c, spin as u8, (spin >> 8) as u8]); // JMP self
    for (i, &b) in s.iter().enumerate() {
        two.cpu.mem.write(0x0320 + i as u16, b);
    }

    // Run the setup to the spin loop (the real firmware's Init + SetMouse run
    // thousands of instructions through the handshake).
    two.cpu.reset();
    two.cpu.pc = 0x0320;
    for _ in 0..500_000 {
        two.cpu.step();
        if two.cpu.pc == spin {
            break;
        }
    }
    assert_eq!(two.cpu.pc, spin, "setup should reach the spin loop");

    // Tick frames, each with a CPU burst — the frame cadence the run loop uses.
    let frames = 5;
    for f in 0..frames {
        // Inject a host movement partway through, to prove the position the
        // firmware reads tracks it.
        if f == 2 {
            two.feed_mouse_delta(250, 100, false);
        }
        two.tick_vbl();
        let mut spent = 0u64;
        while spent < 17_030 {
            two.service_irq();
            spent += two.cpu.step() as u64;
        }
    }

    assert_eq!(read(&mut two, 0x0280), frames, "one IRQ per frame");
    assert_eq!(read(&mut two, 0x0281), 0x08, "ServeMouse reported VBL");
    assert_eq!(
        two.mouse_irq_pending(),
        Some(false),
        "the last ServeMouse de-asserted the line (no spurious re-fire)"
    );
    assert_eq!(
        device_pos(&mut two),
        (250, 100),
        "fed movement reached the mouse"
    );
}
