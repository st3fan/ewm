//! Enhanced //e keyboard (Phase 3b): lower-case input passes through (the ][+
//! upper-cases in the SDL frontend; the //e does not), and the Open-Apple /
//! Solid-Apple keys read as the game-I/O buttons at `$C061`/`$C062`.

use ewm::two::{Two, TwoType};

#[test]
fn open_and_solid_apple_read_as_buttons() {
    let mut two = Two::new(TwoType::Apple2EEnhanced).unwrap();
    // Not pressed.
    assert_eq!(two.cpu.mem.read(0xc061) & 0x80, 0x00);
    assert_eq!(two.cpu.mem.read(0xc062) & 0x80, 0x00);

    // Open-Apple = button 0 ($C061), Solid-Apple = button 1 ($C062).
    two.set_button(0, 0x80);
    two.set_button(1, 0x80);
    assert_eq!(two.cpu.mem.read(0xc061) & 0x80, 0x80, "Open-Apple pressed");
    assert_eq!(two.cpu.mem.read(0xc062) & 0x80, 0x80, "Solid-Apple pressed");

    two.set_button(0, 0x00);
    assert_eq!(two.cpu.mem.read(0xc061) & 0x80, 0x00, "Open-Apple released");
}

struct Machine {
    two: Two,
}

impl Machine {
    fn boot() -> Machine {
        let mut two = Two::new(TwoType::Apple2EEnhanced).expect("apple2e must construct");
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

    fn step_until(&mut self, cap: u64, pred: impl Fn(&Two) -> bool) {
        let mut spent = 0u64;
        while !pred(&self.two) {
            self.step(100_000);
            spent += 100_000;
            assert!(spent < cap, "gave up:\n{}", self.two.text_screen());
        }
    }

    fn type_line(&mut self, line: &str) {
        for &b in line.as_bytes() {
            self.two.key(b);
            self.step_until(2_000_000, |t| t.key_register() & 0x80 == 0);
        }
        self.two.key(0x0d);
        self.step_until(2_000_000, |t| t.key_register() & 0x80 == 0);
    }
}

#[test]
fn lower_case_input_is_preserved() {
    // The //e keyboard latch takes lower case verbatim (no upper-casing).
    // Typing a lower-case string literal echoes and prints it in lower case.
    let mut m = Machine::boot();
    m.step_until(400_000_000, |two| {
        let t = two.text_screen();
        t.contains("DOS VERSION 3.3") && t.contains(']')
    });

    m.type_line("PRINT \"hello\"");
    m.step(3_000_000);

    let text = m.two.text_screen();
    assert!(
        text.lines().any(|l| l.trim() == "hello"),
        "expected lower-case 'hello'; screen was:\n{text}"
    );
}
