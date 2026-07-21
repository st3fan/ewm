//! Headless Apple ][+ tests (the Phase 5 gate): boot the ROMs into
//! AppleSoft, evaluate BASIC via the keyboard latch, and exercise the
//! language card's banking semantics.

use ewm::two::{Two, TwoType};

struct Machine {
    two: Two,
}

impl Machine {
    fn boot() -> Machine {
        let mut two = Two::new(TwoType::Apple2Plus).expect("apple2plus must construct");
        // Since Phase 6 the Disk II is always present (as in ewm_two_init),
        // so booting without a disk hangs in the slot 6 boot ROM like the
        // real machine. Boot the System Master to reach AppleSoft.
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

    /// Step until the predicate holds, with a cycle cap.
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
fn apple2plus_boots_and_evaluates_basic() {
    let mut m = Machine::boot();

    // DOS 3.3 boots and lands at the AppleSoft prompt.
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

#[test]
fn apple2_is_unsupported() {
    // The original NMOS Apple ][ is out of scope; the Enhanced //e (Apple2EEnhanced)
    // now constructs — see two_e_skeleton.rs.
    assert!(Two::new(TwoType::Apple2).is_err());
}

#[test]
fn speaker_toggles_are_cycle_stamped() {
    let mut two = Two::new(TwoType::Apple2Plus).unwrap();
    two.cpu.mem.cycles = 100;
    two.cpu.mem.read(0xc030);
    two.cpu.mem.cycles = 250;
    two.cpu.mem.write(0xc030, 0x00);
    assert_eq!(two.drain_speaker_toggles(), vec![100, 250]);
    assert!(two.drain_speaker_toggles().is_empty());
}

// Language-card semantics, per alc.c. All accesses go straight through the
// memory system like the CPU would.

#[test]
fn language_card_starts_disabled_and_reads_rom() {
    let mut two = Two::new(TwoType::Apple2Plus).unwrap();
    let rom_byte = two.cpu.mem.read(0xd000);
    // Nothing mapped until the first $C08x access.
    two.cpu.mem.write(0xd000, 0x42);
    assert_eq!(two.cpu.mem.read(0xd000), rom_byte);
}

#[test]
fn language_card_write_enable_needs_two_reads() {
    let mut two = Two::new(TwoType::Apple2Plus).unwrap();
    let rom_byte = two.cpu.mem.read(0xd000);

    // One read of $C081 is not enough to write-enable.
    two.cpu.mem.read(0xc081);
    two.cpu.mem.write(0xd000, 0x42);
    two.cpu.mem.read(0xc080); // read-enable bank 2
    assert_eq!(
        two.cpu.mem.read(0xd000),
        0x00,
        "single $C081 read must not enable writes"
    );

    // Two consecutive reads write-enable; reads still come from ROM.
    two.cpu.mem.read(0xc081);
    two.cpu.mem.read(0xc081);
    two.cpu.mem.write(0xd000, 0x42);
    assert_eq!(
        two.cpu.mem.read(0xd000),
        rom_byte,
        "$C081 leaves reads on ROM"
    );

    // $C080: read card RAM, write-protect.
    two.cpu.mem.read(0xc080);
    assert_eq!(two.cpu.mem.read(0xd000), 0x42);
    two.cpu.mem.write(0xd000, 0x99);
    assert_eq!(
        two.cpu.mem.read(0xd000),
        0x42,
        "write after $C080 must be swallowed"
    );
}

#[test]
fn language_card_write_to_switch_resets_count() {
    let mut two = Two::new(TwoType::Apple2Plus).unwrap();

    // read / write / read of $C081: the write resets WRTCOUNT, so writes
    // stay disabled.
    two.cpu.mem.read(0xc081);
    two.cpu.mem.write(0xc081, 0x00);
    two.cpu.mem.read(0xc081);
    two.cpu.mem.write(0xd000, 0x42);
    two.cpu.mem.read(0xc080);
    assert_eq!(two.cpu.mem.read(0xd000), 0x00);
}

#[test]
fn language_card_banks_are_separate_and_e000_is_shared() {
    let mut two = Two::new(TwoType::Apple2Plus).unwrap();

    // Bank 1 ($C08B twice: read + write enable), write $D000 and $E000.
    two.cpu.mem.read(0xc08b);
    two.cpu.mem.read(0xc08b);
    two.cpu.mem.write(0xd000, 0x11);
    two.cpu.mem.write(0xe000, 0x33);

    // Bank 2 ($C083 twice), write $D000.
    two.cpu.mem.read(0xc083);
    two.cpu.mem.read(0xc083);
    two.cpu.mem.write(0xd000, 0x22);
    assert_eq!(two.cpu.mem.read(0xd000), 0x22);
    assert_eq!(
        two.cpu.mem.read(0xe000),
        0x33,
        "$E000 RAM is shared between banks"
    );

    // Back to bank 1: its $D000 contents are intact.
    two.cpu.mem.read(0xc088);
    assert_eq!(two.cpu.mem.read(0xd000), 0x11);
}
