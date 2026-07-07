//! Cycle-accounting accuracy: the Autostart ROM's WAIT routine at $FCA8
//! has an exactly documented cost of (26 + 27·A + 5·A²) / 2 cycles — and
//! its body is SBC-immediate plus taken and untaken branches, the very
//! instructions whose counts the C emulator got wrong. If this matches,
//! the machine runs at a true ≈1.023 MHz.

use ewm::two::{Two, TwoType};

/// The documented cost of `JSR WAIT` with A set is (26 + 27A + 5A²) / 2
/// cycles — a figure that includes the JSR itself. The test enters the
/// routine without one, so it expects 6 cycles less.
fn wait_cycles(a: u64) -> u64 {
    (26 + 27 * a + 5 * a * a) / 2 - 6
}

#[test]
fn rom_wait_routine_has_documented_timing() {
    let mut two = Two::new(TwoType::Apple2Plus).expect("apple2plus must construct");

    for a in [1u8, 10, 100, 200, 255] {
        let cpu = &mut two.cpu;
        cpu.a = a;
        cpu.sp = 0xff;
        cpu.push_word(0x0fff); // RTS returns to $1000
        cpu.pc = 0xfca8; // WAIT

        let start = cpu.counter;
        let mut steps = 0u32;
        while cpu.pc != 0x1000 {
            cpu.step();
            steps += 1;
            assert!(steps < 2_000_000, "WAIT({a}) did not return");
        }
        let used = cpu.counter - start;
        assert_eq!(
            used,
            wait_cycles(a as u64),
            "WAIT({a}): counted {used} cycles, real hardware takes {}",
            wait_cycles(a as u64)
        );
    }
}
