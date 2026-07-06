//! The Phase 6 gate: boot DOS 3.3 from the System Master image, fully
//! headless, and run CATALOG.

use ewm_core::two::{Two, TwoType};

struct Machine {
    two: Two,
}

impl Machine {
    fn boot_with_system_master() -> Machine {
        let mut two = Two::new(TwoType::Apple2Plus).expect("apple2plus must construct");
        two.load_disk(
            0,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../disks/DOS33-SystemMaster.dsk"
            ),
        )
        .expect("cannot load DOS33-SystemMaster.dsk");
        two.cpu.reset();
        Machine { two }
    }

    fn step(&mut self, cycles: u64) {
        let mut done = 0u64;
        while done < cycles {
            done += self.two.cpu.step() as u64;
        }
    }

    fn step_until(&mut self, cap: u64, what: &str, pred: impl Fn(&Two) -> bool) {
        let mut spent = 0u64;
        while !pred(&self.two) {
            self.step(100_000);
            spent += 100_000;
            assert!(
                spent < cap,
                "gave up waiting for {what} after {spent} cycles; screen was:\n{}",
                self.two.text_screen()
            );
        }
    }

    fn type_line(&mut self, line: &str) {
        for &b in line.as_bytes() {
            self.two.key(b);
            self.step_until(2_000_000, "key strobe", |two| {
                two.key_register() & 0x80 == 0
            });
        }
        self.two.key(0x0d);
        self.step_until(2_000_000, "return strobe", |two| {
            two.key_register() & 0x80 == 0
        });
    }
}

#[test]
fn dos33_boots_and_catalogs() {
    let mut m = Machine::boot_with_system_master();

    // The System Master boots DOS 3.3, runs its HELLO program (which prints
    // the DOS banner), and lands at the AppleSoft prompt.
    m.step_until(400_000_000, "the DOS banner", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });

    m.type_line("CATALOG");
    m.step(8_000_000);

    let text = m.two.text_screen();
    assert!(
        text.contains("DISK VOLUME 254"),
        "catalog header missing; screen was:\n{text}"
    );
    assert!(
        text.contains("HELLO"),
        "expected HELLO in the catalog; screen was:\n{text}"
    );
}
