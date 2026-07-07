//! Slot 1 Thunderclock Plus tests: the card's ID bytes and I/O ports, the
//! firmware read routine that deposits the time string at $0200, and a full
//! ProDOS 2.4.3 boot asserting the date/time it derives into its globals.

use ewm::clk::{CLK_READ_ENTRY, ClockTime};
use ewm::two::{Two, TwoType};

/// Monday, 2026-07-06 10:30:59 — inside ProDOS 2.4.3's 2023-2028 year window.
fn sample() -> ClockTime {
    ClockTime {
        month: 7,
        weekday: 1,
        day: 6,
        hour: 10,
        minute: 30,
        second: 59,
    }
}

/// The 18 string bytes the card serves for `sample()`.
fn sample_string() -> [u8; 18] {
    [
        0xb0, 0xb7, 0xac, // 07,
        0xb0, 0xb1, 0xac, // 01, (Monday)
        0xb0, 0xb6, 0xac, // 06,
        0xb1, 0xb0, 0xac, // 10,
        0xb3, 0xb0, 0xac, // 30,
        0xb5, 0xb9, // 59
        0x8d, // CR
    ]
}

fn machine() -> Two {
    Two::new(TwoType::Apple2Plus).expect("apple2plus must construct")
}

/// Call a firmware entry the way the ProDOS driver does: JSR to it and run
/// until it returns (an RTS pops our sentinel return address).
fn call_entry(two: &mut Two, entry: u16) {
    two.cpu.sp = 0xff;
    two.cpu.push_word(0x1233); // RTS returns to $1234
    two.cpu.pc = entry;
    let mut steps = 0;
    while two.cpu.pc != 0x1234 {
        two.cpu.step();
        steps += 1;
        assert!(steps < 100_000, "firmware entry did not return");
    }
}

#[test]
fn rom_has_the_prodos_clock_signature() {
    let mut two = machine();
    let mem = &mut two.cpu.mem;
    // The four ID bytes ProDOS checks to recognize a clock card.
    assert_eq!(mem.read(0xc100), 0x08);
    assert_eq!(mem.read(0xc102), 0x28);
    assert_eq!(mem.read(0xc104), 0x58);
    assert_eq!(mem.read(0xc106), 0x70);
    // Must NOT look like a bootable Disk II card (that signature is
    // $Cn01=$20), or the Autostart scan would try to boot it.
    assert_ne!(mem.read(0xc101), 0x20);
}

#[test]
fn ports_serve_the_time_string() {
    let mut two = machine();
    two.clk_mut().set_fixed_time(sample());
    let mem = &mut two.cpu.mem;

    mem.write(0xc090, 0); // latch
    for (i, want) in sample_string().iter().enumerate() {
        assert_eq!(mem.read(0xc091), *want, "byte {i}");
    }
    // Spent: the data port reads $00 (the firmware's terminator).
    assert_eq!(mem.read(0xc091), 0);
    // A fresh latch restarts the string.
    mem.write(0xc090, 0);
    assert_eq!(mem.read(0xc091), sample_string()[0]);
}

#[test]
fn firmware_read_deposits_the_string_at_0200() {
    let mut two = machine();
    two.clk_mut().set_fixed_time(sample());

    // Poison the buffer so we can see exactly what the firmware writes.
    for addr in 0x0200..0x0220u16 {
        two.cpu.mem.write(addr, 0xee);
    }

    // ProDOS first calls the write/command entry with A = '#'|$80, then read.
    two.cpu.a = 0xa3;
    call_entry(&mut two, 0xc10b); // WRITE entry: must return harmlessly
    call_entry(&mut two, CLK_READ_ENTRY);

    let expected = sample_string();
    for (i, want) in expected.iter().enumerate() {
        assert_eq!(
            two.cpu.mem.read(0x0200 + i as u16),
            *want,
            "$0200+{i} after firmware read"
        );
    }
    // The firmware stops at the terminator: the byte past the string is
    // untouched.
    assert_eq!(two.cpu.mem.read(0x0200 + expected.len() as u16), 0xee);
}

#[test]
fn prodos_243_boot_reads_the_clock() {
    // attach_hdd opens the image writable and ProDOS writes to it during
    // boot, so work on a throwaway copy, not the repo's disk.
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/../disks/ProDOS_2_4_3.po");
    let path = std::env::temp_dir().join(format!("ewm-clk-prodos-{}.po", std::process::id()));
    std::fs::copy(src, &path).expect("cannot copy ProDOS image");

    let mut two = machine();
    two.attach_hdd(path.to_str().unwrap())
        .expect("attach_hdd failed");
    two.clk_mut().set_fixed_time(sample());
    two.cpu.reset();

    // Step until ProDOS has read the clock into its date/time globals. The
    // packed date word for 2026-07-06 is (26<<9)|(7<<5)|6 = $34E6.
    //   $BF90 DATELO, $BF91 DATEHI, $BF92 minute, $BF93 hour
    let settled = |two: &Two| {
        let ram = two.ram();
        ram[0xbf90] == 0xe6 && ram[0xbf91] == 0x34 && ram[0xbf92] == 30 && ram[0xbf93] == 10
    };
    let mut cycles = 0u64;
    while !settled(&two) {
        cycles += two.cpu.step() as u64;
        assert!(
            cycles < 400_000_000,
            "ProDOS never stamped the clock into $BF90-93; globals were \
             {:02x} {:02x} {:02x} {:02x}, screen:\n{}",
            two.ram()[0xbf90],
            two.ram()[0xbf91],
            two.ram()[0xbf92],
            two.ram()[0xbf93],
            two.text_screen()
        );
    }

    std::fs::remove_file(&path).ok();
}
