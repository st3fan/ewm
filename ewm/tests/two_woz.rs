//! WOZ Phase 2 gate: boot DOS 3.3 from the WOZ 1.0 System Master image —
//! through the bit-stream engine, MC3470 fake bits and all — fully headless,
//! and run CATALOG. Mirrors the `.dsk` gates in `two_boot.rs` / `two_dos.rs`.

use ewm::two::{Two, TwoType};

struct Machine {
    two: Two,
}

impl Machine {
    fn boot(path: &str) -> Machine {
        let mut two = Two::new(TwoType::Apple2Plus).expect("apple2plus must construct");
        two.load_disk(0, path).expect("cannot load woz image");
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

fn system_master() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../disks/woz/WOZ 1.0/DOS 3.3 System Master.woz"
    )
    .to_string()
}

#[test]
fn woz_dos33_boots_and_catalogs() {
    let mut m = Machine::boot(&system_master());

    // A WOZ disk streams at real bit speed, so the boot takes real-ish time.
    m.step_until(600_000_000, "the DOS 3.3 banner and prompt", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });

    m.type_line("CATALOG");
    m.step_until(100_000_000, "the catalog listing", |two| {
        let text = two.text_screen();
        text.contains("DISK VOLUME") && text.contains("HELLO")
    });
}
