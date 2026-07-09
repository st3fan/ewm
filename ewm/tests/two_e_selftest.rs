//! Enhanced //e ROM self-test (Phase 8a). Holding Solid-Apple (Closed-Apple,
//! button 1) across a reset runs the built-in diagnostic, which exercises all
//! 128K of RAM, the ROM checksums and the MMU / auxiliary routing, then reports
//! "System OK". A strong end-to-end burn-in for the aux plumbing, fully
//! deterministic and headless.

use ewm::two::{Two, TwoType};

fn step(two: &mut Two, cycles: u64) {
    let mut done = 0u64;
    while done < cycles {
        done += two.cpu.step() as u64;
    }
}

#[test]
fn self_test_reports_system_ok() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    two.set_button(1, 0x80); // Solid-Apple held during reset
    two.cpu.reset();

    let mut spent = 0u64;
    while !two.text_screen().contains("System OK") {
        step(&mut two, 10_000_000);
        spent += 10_000_000;
        assert!(
            spent < 300_000_000,
            "self-test did not report OK; screen was:\n{}",
            two.text_screen()
        );
    }
}
