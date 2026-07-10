//! Enhanced //e analog paddles: the `$C070` PTRIG / `$C064`-`$C065` timer
//! model, ported from the ][+ (`TwoIo`) — previously a documented gap where
//! `IouE::set_joystick` was a no-op, so game controllers moved nothing on
//! the //e while the buttons worked.

use ewm::two::{Two, TwoType};

/// Run the PTRIG/PADL sequence at controlled cycle stamps and return the
/// PADL0 values (immediately after trigger, and after `wait` cycles).
fn paddle_reads(model: TwoType, axis_x: i16, wait: u64) -> (u8, u8) {
    let mut two = Two::new(model).unwrap();
    two.set_joystick(Some((axis_x, 0)));
    two.cpu.mem.cycles = 1_000;
    two.cpu.mem.read(0xc070); // PTRIG: arm the timers
    let armed = two.cpu.mem.read(0xc064);
    two.cpu.mem.cycles = 1_000 + wait;
    let later = two.cpu.mem.read(0xc064);
    (armed, later)
}

#[test]
fn iie_paddle_timer_charges_and_expires() {
    // Centered stick: the timer runs ~128 * 11 cycles; bit 7 is set while
    // charging and clear after expiry.
    let (armed, later) = paddle_reads(TwoType::Apple2E, 0, 3_000);
    assert_eq!(armed, 0xff, "PADL0 charging right after PTRIG");
    assert_eq!(later, 0x00, "PADL0 expired after the timer window");

    // Full-left deflection expires almost immediately; full-right takes the
    // whole window and must still be charging at the same wait.
    let (_, left) = paddle_reads(TwoType::Apple2E, i16::MIN, 200);
    assert_eq!(left, 0x00, "full-left expires quickly");
    let (_, right) = paddle_reads(TwoType::Apple2E, i16::MAX, 2_000);
    assert_eq!(right, 0xff, "full-right still charging");
}

#[test]
fn iie_paddles_match_the_2plus() {
    // The //e port must behave exactly like the ][+ implementation.
    for axis in [i16::MIN, -12_345, 0, 12_345, i16::MAX] {
        for wait in [100, 1_000, 2_000, 4_000] {
            assert_eq!(
                paddle_reads(TwoType::Apple2E, axis, wait),
                paddle_reads(TwoType::Apple2Plus, axis, wait),
                "axis {axis}, wait {wait}"
            );
        }
    }
}

#[test]
fn iie_ptrig_without_joystick_is_inert() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    two.cpu.mem.read(0xc070);
    assert_eq!(
        two.cpu.mem.read(0xc064),
        0x00,
        "no joystick: PADL0 stays low"
    );
}
