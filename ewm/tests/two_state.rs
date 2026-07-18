//! The S3 gate (plans/20260718-01-machine-state.md): full-machine state
//! round-trips through the `Persist` tree. A restored twin must be
//! indistinguishable from the original — same screen, same registers, same
//! media — and must *behave* identically afterwards, which is what catches
//! any component that forgot a field.

use std::collections::BTreeMap;

use ewm::two::{Slot0, Two, TwoType};
use ewm_core::state::{Persist, Reader, Writer};

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

/// Save a machine and restore into `twin`, through the in-memory container.
fn round_trip(two: &Two, twin: &mut Two) {
    let mut w = Writer::new();
    two.save(&mut w);
    let bytes = w.into_bytes();
    let mut r = Reader::new(&bytes);
    twin.restore(&mut r).expect("restore");
    r.done().expect("payload fully consumed");
}

/// Boot DOS 3.3, save at the prompt, restore into a freshly built twin, and
/// prove the twin both *looks* identical (screen, registers, cycle counter)
/// and *behaves* identically: CATALOG on the restored machine reads the
/// restored media through the restored controller and produces the same
/// screen as on the original.
#[test]
fn dos_boot_round_trips_and_catalogs_after_restore() {
    let mut m = Machine::boot_with_system_master();
    m.step_until(400_000_000, "the DOS banner", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });

    let mut twin = Machine::boot_with_system_master(); // fresh, un-run
    round_trip(&m.two, &mut twin.two);

    assert_eq!(twin.two.text_screen(), m.two.text_screen());
    assert_eq!(twin.two.cpu.counter, m.two.cpu.counter);
    assert_eq!(twin.two.cpu.pc, m.two.cpu.pc);
    assert_eq!(twin.two.cpu.a, m.two.cpu.a);
    assert_eq!(twin.two.cpu.sp, m.two.cpu.sp);

    // Behavioural equality: both catalog the (restored) disk identically.
    m.type_line("CATALOG");
    m.step(8_000_000);
    twin.type_line("CATALOG");
    twin.step(8_000_000);
    let text = twin.two.text_screen();
    assert!(
        text.contains("DISK VOLUME 254"),
        "restored machine failed to catalog; screen was:\n{text}"
    );
    assert_eq!(twin.two.text_screen(), m.two.text_screen());
}

/// The //e path: IouE (soft switches, main/aux memory, the built-in
/// language card) and the aux card round-trip. Runs without media — the
/// machine lands in AppleSoft and we park state in both banks first.
#[test]
fn apple2e_round_trip_preserves_switches_and_aux_memory() {
    let build = || {
        let mut two =
            Two::new_with_slots(TwoType::Apple2E, None, Slot0::Language, &BTreeMap::new())
                .expect("apple2e must construct");
        two.cpu.reset();
        Machine { two }
    };

    let mut m = build();
    m.step_until(50_000_000, "the AppleSoft prompt", |two| {
        two.text_screen().contains(']')
    });
    // Park distinctive state: 80-column firmware on (exercises INTCXROM,
    // 80COL, the aux text page) and a variable in BASIC memory.
    m.type_line("PR#3");
    m.step(4_000_000);
    m.type_line("X=42");
    m.step(2_000_000);
    assert!(m.two.col80(), "PR#3 should have enabled 80-column mode");

    let mut twin = build();
    round_trip(&m.two, &mut twin.two);

    assert_eq!(twin.two.text_screen_80(), m.two.text_screen_80());
    assert!(twin.two.col80(), "80COL restored");
    assert_eq!(twin.two.cpu.counter, m.two.cpu.counter);

    // Behavioural equality: PRINT X answers 42 on both, identically.
    m.type_line("PRINT X");
    m.step(2_000_000);
    twin.type_line("PRINT X");
    twin.step(2_000_000);
    let text = twin.two.text_screen_80();
    assert!(
        text.contains("42"),
        "X lost in restore; screen was:\n{text}"
    );
    assert_eq!(twin.two.text_screen_80(), m.two.text_screen_80());
}

/// Restore refuses the wrong model — the cheap seatbelt ahead of the
/// backlog config fingerprint.
#[test]
fn restore_rejects_a_different_model() {
    let plus = Two::new(TwoType::Apple2Plus).expect("construct");
    let mut w = Writer::new();
    plus.save(&mut w);
    let bytes = w.into_bytes();

    let mut e = Two::new_with_slots(TwoType::Apple2E, None, Slot0::Language, &BTreeMap::new())
        .expect("construct");
    let err = e
        .restore(&mut Reader::new(&bytes))
        .expect_err("model mismatch must be rejected")
        .to_string();
    assert!(err.contains("2plus"), "{err}");
}

/// The S4 file surface: `Two::save_state` / `restore_state` round-trip
/// through an actual state file, atomically.
#[test]
fn state_file_round_trip() {
    let dir = std::env::temp_dir().join(format!("ewm-state-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("test dir");
    let path = dir.join("machine.state");
    let path = path.to_str().expect("utf-8 path");

    let mut m = Machine::boot_with_system_master();
    m.step_until(400_000_000, "the DOS banner", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });
    // The e2e from the plan: park a variable, save, restore, ask for it.
    m.type_line("X=42");
    m.step(2_000_000);
    m.two.save_state(path).expect("save");

    let mut twin = Machine::boot_with_system_master();
    twin.two.restore_state(path).expect("restore");
    twin.type_line("PRINT X");
    twin.step(2_000_000);
    let text = twin.two.text_screen();
    assert!(
        text.contains("42"),
        "X lost across the file; screen was:\n{text}"
    );

    // Corrupt file: rejected, machine not run.
    std::fs::write(path, b"garbage").expect("clobber");
    let mut broken = Machine::boot_with_system_master();
    assert!(broken.two.restore_state(path).is_err());

    std::fs::remove_dir_all(&dir).ok();
}

/// The S5 determinism gate (notes/STATE.md §8), the state analogue of the
/// golden-BMP tests: save **mid-boot** — motor on, arm seeking, DOS half
/// loaded, the harshest moment — restore into a fresh twin, then run BOTH
/// machines millions of further cycles. If any component forgot a field,
/// the trajectories diverge and the screens (text and rendered pixels)
/// stop matching.
#[test]
fn mid_boot_restore_is_deterministic_for_seconds_of_execution() {
    use ewm::scr::{PixelLayout, SCR_HEIGHT, Scr, frame_width};

    let mut m = Machine::boot_with_system_master();
    m.step(3_000_000); // ~3 emulated seconds into the boot: mid-load

    let mut twin = Machine::boot_with_system_master();
    round_trip(&m.two, &mut twin.two);
    assert_eq!(twin.two.cpu.counter, m.two.cpu.counter);

    // Run both to the DOS prompt and beyond, comparing along the way.
    for leg in 0..5 {
        m.step(2_000_000);
        twin.step(2_000_000);
        assert_eq!(
            twin.two.text_screen(),
            m.two.text_screen(),
            "trajectories diverged on leg {leg}"
        );
        assert_eq!(twin.two.cpu.counter, m.two.cpu.counter);
        assert_eq!(twin.two.cpu.pc, m.two.cpu.pc);
    }
    assert!(
        m.two.text_screen().contains("DOS VERSION 3.3"),
        "the original should have finished booting; screen was:\n{}",
        m.two.text_screen()
    );

    // Pixel-level equality through the pure renderer, both machines.
    let mut scr_m = Scr::new(PixelLayout::Argb8888);
    let mut scr_t = Scr::new(PixelLayout::Argb8888);
    scr_m.update(&m.two, 0, 30);
    scr_t.update(&twin.two, 0, 30);
    assert_eq!(
        scr_m.frame(m.two.model()),
        scr_t.frame(twin.two.model()),
        "rendered framebuffers diverged"
    );
    assert_eq!(frame_width(m.two.model()) * SCR_HEIGHT, 280 * 192);
}
