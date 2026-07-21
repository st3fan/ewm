//! Headless Enhanced Apple //e skeleton (Phase 2a): the 65C02 fetches the //e
//! reset vector and executes system ROM. It cannot finish booting yet — the
//! internal `$CX` ROM arbitration is Phase 2b and the soft switches are Phase
//! 2c — so this only proves the machine constructs and runs real //e code.

use ewm::two::{Two, TwoType};

#[test]
fn apple2e_constructs_and_enters_cold_start() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).expect("apple2e must construct");
    assert_eq!(two.model(), TwoType::Apple2EEnhanced);

    two.cpu.reset();

    // The //e reset vector ($FFFC) points at the monitor RESET entry $FA62,
    // whose first instruction is CLD ($D8). Reaching it proves the banked //e
    // ROM is mapped and readable through the language card.
    assert_eq!(
        two.cpu.pc, 0xfa62,
        "reset vector should enter //e cold start"
    );
    assert_eq!(
        two.cpu.mem.read(0xfa62),
        0xd8,
        "cold start should begin with CLD"
    );

    // Run a budget of //e ROM without panicking. The 65C02 should execute the
    // monitor cold start in the banked ROM ($E000-$FFFF), then wander into the
    // internal $C300 firmware space — which is unmapped until Phase 2b, so it
    // gets stuck there. Reaching the monitor proves it ran real //e ROM.
    let mut ran_monitor_rom = false;
    for _ in 0..500_000 {
        two.cpu.step();
        if two.cpu.pc >= 0xe000 {
            ran_monitor_rom = true;
        }
    }
    assert!(
        ran_monitor_rom,
        "cpu should have executed //e monitor ROM ($E000-$FFFF); pc=${:04X}",
        two.cpu.pc
    );
    assert_ne!(
        two.cpu.pc, 0xfa62,
        "cpu should have advanced past the reset entry"
    );
}
