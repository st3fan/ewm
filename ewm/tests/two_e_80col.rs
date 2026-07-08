//! Enhanced //e 80-column text (Phase 5b). 80COL (`$C00C`/`$C00D`) turns the
//! 40-column display into 80 columns by interleaving the two banks: aux holds
//! the even display columns (0, 2, …, 78), main the odd (1, 3, …, 79). The
//! `text_screen_80()` scrape and the 560-wide renderer both read that
//! interleave. Verified against the ROM's own `PR#3` output.

use ewm::scr::{PixelLayout, SCR_HEIGHT, SCR_WIDTH_E, Scr, encode_bmp};
use ewm::two::{Two, TwoType};

const RAMWRT_OFF: u16 = 0xc004;
const RAMWRT_ON: u16 = 0xc005;
const COL80_ON: u16 = 0xc00d;
const ALTCHARSET_ON: u16 = 0xc00f;
const TEXT: u16 = 0xc051;

fn set(two: &mut Two, addr: u16) {
    two.cpu.mem.write(addr, 0);
}

/// Lay `text` across 80-column row `row`, honoring the aux-even / main-odd
/// interleave: even columns are written to aux (RAMWRT on), odd to main. The
/// bytes are normal screen codes (ASCII with the high bit set).
fn put_80(two: &mut Two, row: usize, text: &str) {
    let base = (0x400 + 0x80 * (row % 8) + 0x28 * (row / 8)) as u16;
    for (col, ch) in text.chars().enumerate().take(80) {
        set(two, if col % 2 == 0 { RAMWRT_ON } else { RAMWRT_OFF });
        two.cpu
            .mem
            .write(base + (col / 2) as u16, (ch as u8) | 0x80);
    }
    set(two, RAMWRT_OFF);
}

#[test]
fn text_screen_80_reads_the_interleave() {
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, COL80_ON);
    set(&mut two, ALTCHARSET_ON);

    let line = "The quick brown fox jumps over the lazy dog 0123456789 ABCDEF!";
    put_80(&mut two, 0, line);

    let first = two.text_screen_80().lines().next().unwrap().to_string();
    assert_eq!(
        &first[..line.len()],
        line,
        "80-column scrape reads interleave"
    );
    assert!(
        first.chars().any(|c| c.is_ascii_lowercase()),
        "lower case present in the 80-column scrape"
    );

    // RD80COL ($C01F) reports the switch.
    assert_eq!(two.cpu.mem.read(0xc01f) & 0x80, 0x80, "RD80COL on");
}

#[test]
fn even_and_odd_columns_come_from_the_expected_bank() {
    // Directly prove which bank feeds which column: write only the aux bank and
    // confirm the scrape shows those glyphs at even columns and blanks at odd.
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, COL80_ON);
    let base = 0x400u16;
    // aux (RAMWRT on): 'X' at byte 0 -> display column 0.
    set(&mut two, RAMWRT_ON);
    two.cpu.mem.write(base, b'X' | 0x80);
    // main (RAMWRT off): 'Y' at byte 0 -> display column 1.
    set(&mut two, RAMWRT_OFF);
    two.cpu.mem.write(base, b'Y' | 0x80);

    let first = two.text_screen_80();
    let mut chars = first.chars();
    assert_eq!(chars.next().unwrap(), 'X', "column 0 comes from aux");
    assert_eq!(chars.next().unwrap(), 'Y', "column 1 comes from main");
}

/// Boot DOS 3.3, wait for the AppleSoft prompt, and drive keystrokes.
struct Machine {
    two: Two,
}
impl Machine {
    fn boot() -> Machine {
        let mut two = Two::new(TwoType::Apple2E).unwrap();
        two.load_disk(
            0,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../disks/DOS33-SystemMaster.dsk"
            ),
        )
        .unwrap();
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
            self.step_until(3_000_000, |t| t.key_register() & 0x80 == 0);
        }
        self.two.key(0x0d);
        self.step_until(3_000_000, |t| t.key_register() & 0x80 == 0);
    }
}

#[test]
fn pr3_enables_80_columns_and_prints() {
    // The firmware path: PR#3 activates the 80-column firmware, which turns on
    // 80COL and ALTCHARSET, so a printed string lands across 80 columns and
    // lower case finally displays.
    let mut m = Machine::boot();
    m.step_until(400_000_000, |two| {
        let t = two.text_screen();
        t.contains("DOS VERSION 3.3") && t.contains(']')
    });
    assert!(!m.two.col80(), "40 columns before PR#3");

    m.type_line("PR#3");
    m.step(5_000_000);
    assert!(m.two.col80(), "PR#3 turns on 80COL");
    assert!(m.two.alt_charset(), "PR#3 turns on ALTCHARSET (lower case)");

    m.type_line("PRINT \"Hello World in lower case\"");
    m.step(5_000_000);
    let text = m.two.text_screen_80();
    assert!(
        text.lines()
            .any(|l| l.trim() == "Hello World in lower case"),
        "80-column output shows the mixed-case string; screen was:\n{text}"
    );
}

#[test]
fn eighty_col_screen_matches_golden_bmp() {
    // A deterministic 80-column scene rendered through the 560-wide path.
    let mut two = Two::new(TwoType::Apple2E).unwrap();
    set(&mut two, TEXT);
    set(&mut two, COL80_ON);
    set(&mut two, ALTCHARSET_ON);
    // Clear the screen to spaces first (fresh RAM is $00, which is inverse '@'
    // in the alternate set), then lay out the scene.
    let blank = " ".repeat(80);
    for row in 0..24 {
        put_80(&mut two, row, &blank);
    }
    put_80(
        &mut two,
        0,
        "The quick brown fox jumps over the lazy dog. 0123456789",
    );
    put_80(
        &mut two,
        2,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz !@#$%^&*()",
    );
    put_80(
        &mut two,
        23,
        "80 columns: 0        1         2         3         4         5   ",
    );

    let mut scr = Scr::new(PixelLayout::Argb8888);
    scr.update(&two, 0, 40);
    let bmp = encode_bmp(scr.frame(TwoType::Apple2E), SCR_WIDTH_E, SCR_HEIGHT);

    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/two-e-80col.bmp");
    if std::env::var("EWM_WRITE_GOLDEN").is_ok() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/golden")).unwrap();
        std::fs::write(golden_path, &bmp).unwrap();
        return;
    }
    match std::fs::read(golden_path) {
        Ok(golden) => assert_eq!(bmp, golden, "80-column screen differs from the golden BMP"),
        Err(_) => panic!(
            "golden BMP missing — generate it with:\n  \
             EWM_WRITE_GOLDEN=1 cargo test -p ewm eighty_col_screen_matches_golden_bmp"
        ),
    }
}
