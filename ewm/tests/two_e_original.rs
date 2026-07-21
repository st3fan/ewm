//! Headless original (unenhanced) Apple //e boot test (E3 of
//! plans/20260720-02): a bare 6502 //e — the 342-0134/0135 system ROMs, no
//! disk controller — cold-boots to the Applesoft `]` prompt. This is the
//! functional gate that the unenhanced ROMs run on the 6502, the mirror of
//! the Enhanced //e's `two_e_boot` gate.

use std::collections::BTreeMap;

use ewm::two::{Slot0, Two, TwoType};

#[test]
fn original_iie_cold_boots_to_applesoft_prompt() {
    // No disk controller in the slot table, so the Autostart cold-start drops
    // straight into Applesoft rather than spinning an empty Disk II forever.
    let mut two = Two::new_with_slots(TwoType::Apple2E, None, Slot0::Language, &BTreeMap::new())
        .expect("original //e must construct");
    assert_eq!(two.model(), TwoType::Apple2E);
    two.cpu.reset();

    let mut spent = 0u64;
    loop {
        for _ in 0..100_000 {
            two.cpu.step();
        }
        spent += 100_000;
        if two.text_screen().contains(']') {
            break;
        }
        assert!(
            spent < 20_000_000,
            "gave up waiting for the ] prompt after {spent} cycles; screen was:\n{}",
            two.text_screen()
        );
    }
}
