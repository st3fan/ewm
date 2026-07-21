//! Enhanced //e end-to-end ProDOS boot (Phase 8b). Booting ProDOS 2.4.3 on the
//! //e exercises the auxiliary memory and MMU with a real, aux-heavy operating
//! system (ProDOS keeps its global page, buffers and /RAM disk in aux), so the
//! Bitsy Bye launcher listing the volume is a strong integration burn-in.
//!
//! Note: Bitsy Bye comes up in 40 columns in this configuration; the 80-column
//! text path itself is gated by `two_e_80col` (Phase 5b) and the `PR#3` test.

use ewm::two::{Two, TwoType};

fn step(two: &mut Two, cycles: u64) {
    let mut done = 0u64;
    while done < cycles {
        done += two.cpu.step() as u64;
    }
}

#[test]
fn prodos_boots_to_the_bitsy_bye_launcher() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    two.load_disk(
        0,
        concat!(env!("CARGO_MANIFEST_DIR"), "/../disks/ProDOS_2_4_3.po"),
    )
    .expect("cannot load ProDOS_2_4_3.po");
    two.cpu.reset();

    let mut spent = 0u64;
    while !two.text_screen().contains("PRODOS.2.4.3") {
        step(&mut two, 10_000_000);
        spent += 10_000_000;
        assert!(
            spent < 250_000_000,
            "ProDOS did not reach the Bitsy Bye launcher; screen was:\n{}",
            two.text_screen()
        );
    }

    // The launcher lists the mounted volume and its files.
    let screen = two.text_screen();
    assert!(
        screen.contains("BITSY.BOOT"),
        "expected the Bitsy Bye file listing; screen was:\n{screen}"
    );
}
