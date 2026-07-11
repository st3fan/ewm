//! End-to-end DHGR gate: boot Total Replay on the //e (slot 7 hard drive +
//! ProDOS + 128K aux detection), launch Thexder from the menu, and assert
//! the machine switches into double hi-res — the integration that found the
//! aligned-cell DHGR readability bug. Skips when the (untracked, commercial)
//! image is absent, like the protected WOZ gates.

use ewm::two::{GraphicsMode, ScreenMode, Two, TwoType};

#[test]
fn total_replay_thexder_engages_dhgr() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../disks/Total Replay v6.0.1.hdv"
    );
    if !std::path::Path::new(path).exists() {
        eprintln!("skipping: Total Replay v6.0.1.hdv not present");
        return;
    }

    let mut two = Two::new(TwoType::Apple2E).unwrap();
    two.attach_hdd(path).expect("attach_hdd failed");
    two.cpu.reset();

    let step = |two: &mut Two, cycles: u64| {
        let mut done = 0u64;
        while done < cycles {
            done += two.cpu.step() as u64;
        }
    };

    // Boot to the menu, then incremental-search THEXDER and launch it.
    step(&mut two, 60_000_000);
    for b in b"THEXDER" {
        two.key(*b);
        step(&mut two, 500_000);
    }
    two.key(0x0d);

    // The title screen must arrive in double hi-res.
    let mut spent = 0u64;
    loop {
        step(&mut two, 5_000_000);
        spent += 5_000_000;
        if two.screen_mode() == ScreenMode::Graphics
            && two.screen_graphics_mode() == GraphicsMode::Hgr
            && two.col80()
            && two.dhires()
        {
            break;
        }
        assert!(
            spent < 120_000_000,
            "Thexder never engaged DHGR; col80={} dhires={} mode={:?}",
            two.col80(),
            two.dhires(),
            two.screen_mode()
        );
    }
}
