//! The AppleMouse II flagship (plans/20260721-03 P3): host pointer input
//! reaching a program through the **real firmware on a //e** — MousePaint's
//! exact environment, and the path that hung with the synthetic card.
//!
//! On the //e the card's banked `$Cn00` ROM is served through the IOU's
//! `$CX`-ROM region (the mouse device shadows it), so this also proves the
//! banking works under the //e's INTCXROM arbitration. The committable gate is
//! this firmware-level assertion; booting the actual MousePaint disk is a
//! dev-time check (the disk is not redistributable).

use std::collections::BTreeMap;

use ewm::aux;
use ewm::two::{Slot0, SlotDevice, Two, TwoType};

/// An Enhanced //e (with the ext-80-col aux card, as `examples/mouse.json`
/// ships) whose slot 4 holds a mouse.
fn iie_with_mouse() -> Two {
    Two::new_with_slots(
        TwoType::Apple2EEnhanced,
        Some(aux::parse("ext80col").unwrap()),
        Slot0::Language,
        &BTreeMap::from([(4, SlotDevice::Mouse)]),
    )
    .expect("an Enhanced //e with a slot-4 mouse must construct")
}

fn read(two: &mut Two, addr: u16) -> u8 {
    two.cpu.mem.read(addr)
}

/// The low byte of routine `k` from the `$C412` offset table → its `$C4xx`
/// entry.
fn entry(two: &mut Two, k: u16) -> u16 {
    0xc400 | read(two, 0xc412 + k) as u16
}

/// Set X=$C4 / Y=$40 (slot-4 firmware register convention) before a JSR.
fn ldxy_jsr(p: &mut Vec<u8>, addr: u16) {
    p.extend([0xa2, 0xc4, 0xa0, 0x40, 0x20, addr as u8, (addr >> 8) as u8]);
}
fn lda_ldxy_jsr(p: &mut Vec<u8>, imm: u8, addr: u16) {
    p.extend([0xa9, imm]);
    ldxy_jsr(p, addr);
}

/// Run the CPU until it reaches `target` (or panic if it runs away) — the
/// firmware has no timeouts, so a handshake bug would hang here.
fn run_to(two: &mut Two, target: u16, what: &str) {
    for _ in 0..1_000_000 {
        two.cpu.step();
        if two.cpu.pc == target {
            return;
        }
    }
    panic!("firmware ran away before {what} (pc=${:04x})", two.cpu.pc);
}

#[test]
fn iie_identifies_the_card_from_the_banked_rom() {
    // The banked $Cn00 ROM, served through the //e IOU's $CX-ROM region.
    let mut two = iie_with_mouse();
    assert_ne!(read(&mut two, 0xc401), 0x20, "not a Disk II boot signature");
    assert_eq!(read(&mut two, 0xc405), 0x38);
    assert_eq!(read(&mut two, 0xc407), 0x18);
    assert_eq!(read(&mut two, 0xc40c), 0x20);
    assert_eq!(read(&mut two, 0xc4fb), 0xd6, "AppleMouse ID");
}

#[test]
fn iie_firmware_reports_host_pointer_input() {
    // The flagship: on the //e, a host pointer position + button fed from the
    // frontend reaches a program through the real ROM's ReadMouse — the exact
    // sequence MousePaint runs (and which hung with the synthetic card).
    let mut two = iie_with_mouse();
    let set_mouse = entry(&mut two, 0);
    let read_mouse = entry(&mut two, 2);
    let init_mouse = entry(&mut two, 7);

    // $0300: InitMouse; SetMouse mode 1 (mouse on); park. Then at `read_at`:
    // ReadMouse; park. Host input is fed from Rust between the two parks.
    let mut p: Vec<u8> = Vec::new();
    ldxy_jsr(&mut p, init_mouse);
    lda_ldxy_jsr(&mut p, 0x01, set_mouse);
    let setup_done = 0x0300 + p.len() as u16;
    p.extend([0x4c, setup_done as u8, (setup_done >> 8) as u8]);
    let read_at = 0x0300 + p.len() as u16;
    ldxy_jsr(&mut p, read_mouse);
    let park = 0x0300 + p.len() as u16;
    p.extend([0x4c, park as u8, (park >> 8) as u8]);
    for (i, &b) in p.iter().enumerate() {
        two.cpu.mem.write(0x0300 + i as u16, b);
    }

    two.cpu.reset();
    two.cpu.pc = 0x0300;
    run_to(&mut two, setup_done, "InitMouse+SetMouse");

    // The frontend feeds an absolute pointer: pixel (200, 100) of a 280×192
    // surface, mapped into the default clamp window, with the button down.
    two.feed_mouse_pixel(200, 100, true, 280, 192);
    let (fx, fy) = two.mouse_position().expect("the machine has a mouse");
    assert!(fx > 0 && fy > 0, "the fed pixel mapped into the window");

    two.cpu.pc = read_at;
    run_to(&mut two, park, "ReadMouse");

    // ReadMouse deposits X/Y into the slot-4 holes and the status byte; the
    // firmware reports exactly what the host fed.
    let x = u16::from_le_bytes([read(&mut two, 0x047c), read(&mut two, 0x057c)]);
    let y = u16::from_le_bytes([read(&mut two, 0x04fc), read(&mut two, 0x05fc)]);
    assert_eq!(
        (x as i16, y as i16),
        (fx, fy),
        "firmware reports the fed position"
    );
    let status = read(&mut two, 0x077c);
    assert_eq!(status & 0x80, 0x80, "button-down reached the program");
    assert_eq!(
        status & 0x20,
        0x20,
        "movement-since-last-read reached the program"
    );
}
