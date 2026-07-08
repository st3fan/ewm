//! Headless Enhanced Apple //e boot test (Phase 2c): with the `$C000-$C01F`
//! soft switches fleshed out, the //e boots DOS 3.3 to the AppleSoft `]`
//! prompt and evaluates BASIC through the keyboard latch — the same flow as
//! the Apple ][+ `two_boot` gate. DOS 3.3 runs on a //e.

use ewm::two::{Two, TwoType};

struct Machine {
    two: Two,
}

impl Machine {
    fn boot() -> Machine {
        let mut two = Two::new(TwoType::Apple2E).expect("apple2e must construct");
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

    /// Type a line through the keyboard latch, waiting for the strobe to be
    /// consumed after each key.
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
fn apple2e_boots_dos33_and_evaluates_basic() {
    let mut m = Machine::boot();

    // DOS 3.3 boots (loading Integer BASIC into the language card on the way)
    // and lands at the AppleSoft prompt.
    m.step_until(400_000_000, "the ] prompt", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });

    m.type_line("PRINT 2+2");
    m.step(2_000_000);

    let text = m.two.text_screen();
    assert!(
        text.lines().any(|l| l.trim() == "4"),
        "expected the answer 4; screen was:\n{text}"
    );
}

// --- $C000-$C01F soft-switch state / status-read round-trips ---

/// Write a switch (write-to-set) and read a status register through the bus.
fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}
fn status(two: &mut Two, addr: u16) -> u8 {
    two.cpu.mem.read(addr) & 0x80
}

#[test]
fn memory_switch_round_trips() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    // Each switch pair (off/on) is reflected by its RD status register in bit 7.
    for &(off, on, rd) in &[
        (0xc000u16, 0xc001u16, 0xc018u16), // 80STORE  -> RD80STORE
        (0xc002, 0xc003, 0xc013),          // RAMRD    -> RDRAMRD
        (0xc004, 0xc005, 0xc014),          // RAMWRT   -> RDRAMWRT
        (0xc008, 0xc009, 0xc016),          // ALTZP    -> RDALTZP
        (0xc00c, 0xc00d, 0xc01f),          // 80COL    -> RD80COL
        (0xc00e, 0xc00f, 0xc01e),          // ALTCHARSET -> RDALTCHAR
    ] {
        set(&mut two, on);
        assert_eq!(status(&mut two, rd), 0x80, "on: ${on:04X} -> ${rd:04X}");
        set(&mut two, off);
        assert_eq!(status(&mut two, rd), 0x00, "off: ${off:04X} -> ${rd:04X}");
    }
}

#[test]
fn display_switch_round_trips() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    // Display switches toggle on any access; their RD registers report state.
    for &(off, on, rd) in &[
        (0xc050u16, 0xc051u16, 0xc01au16), // TEXT  -> RDTEXT
        (0xc052, 0xc053, 0xc01b),          // MIXED -> RDMIXED
        (0xc054, 0xc055, 0xc01c),          // PAGE2 -> RDPAGE2
        (0xc056, 0xc057, 0xc01d),          // HIRES -> RDHIRES
    ] {
        let _ = two.cpu.mem.read(on);
        assert_eq!(status(&mut two, rd), 0x80, "on: ${on:04X} -> ${rd:04X}");
        let _ = two.cpu.mem.read(off);
        assert_eq!(status(&mut two, rd), 0x00, "off: ${off:04X} -> ${rd:04X}");
    }
}

#[test]
fn keyboard_strobe_clears_on_read_or_write() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    // A latched key shows in bit 7 of $C000; reading $C010 clears the strobe.
    two.key(b'A');
    assert_eq!(two.cpu.mem.read(0xc000) & 0x80, 0x80);
    let _ = two.cpu.mem.read(0xc010);
    assert_eq!(
        two.key_register() & 0x80,
        0x00,
        "read $C010 clears the strobe"
    );

    // The //e firmware also clears it with a write (STA $C010).
    two.key(b'B');
    assert_eq!(two.cpu.mem.read(0xc000) & 0x80, 0x80);
    two.cpu.mem.write(0xc010, 0);
    assert_eq!(
        two.key_register() & 0x80,
        0x00,
        "write $C010 clears the strobe"
    );
}
