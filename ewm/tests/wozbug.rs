//! The WozBug workflow test: the PR #253 RWTS hunt, replayed with tools
//! instead of temporary code. Boot DOS 3.3, set a breakpoint at RWTS by
//! symbol, type CATALOG, land in the debugger, inspect, resume.

use ewm::two::{Two, TwoType};
use ewm::wozbug::WozBug;

const DOS33: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../disks/DOS33-SystemMaster.dsk"
);

fn step_until(two: &mut Two, cap: u64, what: &str, pred: impl Fn(&Two) -> bool) {
    let mut cycles = 0u64;
    while !pred(two) {
        assert!(
            cycles < cap,
            "gave up waiting for {what} after {cycles} cycles; screen was:\n{}",
            two.text_screen()
        );
        let target = cycles + 50_000;
        while cycles < target {
            let spent = two.cpu.step() as u64;
            if spent == 0 {
                return; // stopped on a breakpoint
            }
            cycles += spent;
        }
    }
}

fn type_line(two: &mut Two, line: &str) {
    for &b in line.as_bytes() {
        two.key(b);
        step_until(two, 2_000_000, "key strobe", |two| {
            two.key_register() & 0x80 == 0
        });
    }
    two.key(0x0d);
    step_until(two, 2_000_000, "return strobe", |two| {
        two.key_register() & 0x80 == 0
    });
}

#[test]
fn catalog_lands_on_the_rwts_breakpoint() {
    let mut wb = WozBug::new();
    let mut two = Two::new(TwoType::Apple2Plus).expect("machine must construct");
    two.load_disk(0, DOS33).expect("load DOS 3.3");
    two.cpu.reset();
    step_until(&mut two, 400_000_000, "the DOS banner", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });

    // Arm the breakpoint by symbol, then ask DOS for a catalog.
    assert_eq!(
        wb.execute(&mut two, "B RWTS"),
        "breakpoint set at BD00 (RWTS)"
    );
    type_line(&mut two, "CATALOG");
    step_until(&mut two, 60_000_000, "the RWTS breakpoint", |two| {
        two.cpu.stopped()
    });
    assert!(two.cpu.stopped(), "CATALOG must call RWTS");

    // Inspect: registers name the landing site, DSK shows the controller.
    let r = wb.execute(&mut two, "R");
    assert!(r.contains("PC=BD00 (RWTS)"), "{r}");
    assert!(r.contains("[stopped]"), "{r}");
    let dsk = wb.execute(&mut two, "DSK");
    assert!(dsk.contains("S6:"), "{dsk}");
    assert!(dsk.contains("D1 loaded"), "{dsk}");

    // The IOB the Monitor-era docs promise: Y/A point at it on entry.
    let iob = ((two.cpu.a as u16) << 8) | two.cpu.y as u16;
    assert_eq!(iob, 0xb7e8, "RWTS entered with Y/A -> IOB");

    // Single-step off the breakpoint, then clear it and run to the
    // catalog — the machine is unharmed by the detour.
    let s = wb.execute(&mut two, "S");
    assert!(s.contains("BD00:"), "{s}");
    wb.execute(&mut two, "B-");
    wb.execute(&mut two, "G");
    step_until(&mut two, 60_000_000, "the catalog", |two| {
        two.text_screen().contains("DISK VOLUME 254")
    });
}
