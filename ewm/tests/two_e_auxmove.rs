//! Enhanced //e aux-memory round-trip through the real ROM firmware (Phase
//! 4c integration burn-in). Drives the Monitor's `AUXMOVE` block-move
//! primitive (`$C311`) to copy a buffer main -> aux and back, proving the
//! auxiliary-RAM plumbing (RAMRD/RAMWRT, Phase 4a) works with actual firmware.
//! AUXMOVE lives in the internal `$C3xx` ROM, so INTCXROM must be on.

use ewm::two::{Two, TwoType};

const SETINTCXROM: u16 = 0xc007;

/// Assemble a tiny driver: load the AUXMOVE pointers (A1 = source start at
/// `$3C`, A2 = source end at `$3E`, A4 = destination at `$42`), set the carry
/// for the direction (`SEC` = main -> aux, `CLC` = aux -> main), `JSR $C311`,
/// then `RTS`.
fn auxmove_program(src: u16, end: u16, dst: u16, to_aux: bool) -> Vec<u8> {
    let mut p = Vec::new();
    let mut set = |zp: u8, v: u8| {
        p.extend_from_slice(&[0xa9, v, 0x85, zp]); // LDA #v ; STA zp
    };
    set(0x3c, src as u8);
    set(0x3d, (src >> 8) as u8);
    set(0x3e, end as u8);
    set(0x3f, (end >> 8) as u8);
    set(0x42, dst as u8);
    set(0x43, (dst >> 8) as u8);
    p.push(if to_aux { 0x38 } else { 0x18 }); // SEC / CLC
    p.extend_from_slice(&[0x20, 0x11, 0xc3]); // JSR $C311 (AUXMOVE)
    p.push(0x60); // RTS
    p
}

/// Load `prog` at `org` and run it to completion: a sentinel return address is
/// pushed so the program's final `RTS` lands on `$1234`, which ends the loop.
fn run(two: &mut Two, prog: &[u8], org: u16) {
    for (i, &b) in prog.iter().enumerate() {
        two.cpu.mem.write(org + i as u16, b);
    }
    two.cpu.sp = 0xff;
    two.cpu.push_word(0x1233); // RTS -> $1234
    two.cpu.pc = org;
    let mut steps = 0;
    while two.cpu.pc != 0x1234 {
        two.cpu.step();
        steps += 1;
        assert!(steps < 100_000, "program did not return");
    }
}

#[test]
fn auxmove_round_trips_a_buffer_main_to_aux_and_back() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    two.cpu.mem.write(SETINTCXROM, 0); // $C3xx = internal ROM so AUXMOVE is reachable

    // Seed a recognisable pattern in main $0300-$030F (RAMWRT off = main).
    for i in 0..16u16 {
        two.cpu.mem.write(0x0300 + i, 0x10 + i as u8);
    }

    // main -> aux
    let prog = auxmove_program(0x0300, 0x030f, 0x0300, true);
    run(&mut two, &prog, 0x0800);
    for i in 0..16usize {
        assert_eq!(
            two.aux_ram()[0x0300 + i],
            0x10 + i as u8,
            "aux[{i}] after main->aux"
        );
    }

    // Wipe main, confirm it is gone, then aux -> main.
    for i in 0..16u16 {
        two.cpu.mem.write(0x0300 + i, 0x00);
    }
    assert!(
        two.ram()[0x0300..0x0310].iter().all(|&b| b == 0),
        "main buffer cleared before the copy back"
    );

    let prog = auxmove_program(0x0300, 0x030f, 0x0300, false);
    run(&mut two, &prog, 0x0800);
    for i in 0..16usize {
        assert_eq!(
            two.ram()[0x0300 + i],
            0x10 + i as u8,
            "main[{i}] survived the round-trip through aux"
        );
    }
}
