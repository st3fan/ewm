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

// --- Protection burn-ins (Phase 3): a deterministic subset of the sweep ---
// (the full sweep lives in zz_woz_sweep.rs, run manually with --ignored).

fn boots_to_graphics(model: TwoType, name: &str, cap: u64) {
    let path = format!(
        "{}/../disks/woz/WOZ 1.0/{}.woz",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    // The protected reference images are not committed (only Apple system
    // software is, per repo precedent) — exercise them when present locally.
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {name}.woz not present");
        return;
    }
    let mut two = Two::new(model).unwrap();
    two.load_disk(0, &path).expect("cannot load woz image");
    two.cpu.reset();
    let mut spent = 0u64;
    while two.screen_mode() != ewm::two::ScreenMode::Graphics {
        let mut done = 0u64;
        while done < 1_000_000 {
            done += two.cpu.step() as u64;
        }
        spent += 1_000_000;
        assert!(
            spent < cap,
            "{name} did not reach graphics; screen was:\n{}",
            two.text_screen()
        );
    }
}

/// The E7 protection (D5 E7 E7 E7 + `$C08D` sequencer-reset re-framing).
#[test]
fn woz_commando_boots() {
    boots_to_graphics(TwoType::Apple2Plus, "Commando - Disk 1, Side A", 30_000_000);
}

/// Half-track stepping (odd TMAP positions with real content).
#[test]
fn woz_bilestoad_boots() {
    boots_to_graphics(
        TwoType::Apple2Plus,
        "The Bilestoad - Disk 1, Side A",
        30_000_000,
    );
}

/// RWTS18 (Broderbund 18-sector format) on the 128K machine it requires.
#[test]
fn woz_wings_of_fury_boots_on_iie() {
    boots_to_graphics(
        TwoType::Apple2E,
        "Wings of Fury - Disk 1, Side A",
        60_000_000,
    );
}
