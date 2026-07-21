//! Slot-table machine tests (JSON_CONFIG Phase B): multiple Disk ][
//! controllers with independent state, the Autostart boot-scan order,
//! empty slots, and clock / hard-drive cards moved out of their classic
//! slots — all through `Two::new_with_slots`.

use std::collections::BTreeMap;

use ewm::clk::{ClockTime, clk_read_entry};
use ewm::hdd::hdd_driver_entry;
use ewm::two::{Slot0, SlotDevice, Two, TwoType};

const DOS33: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../disks/DOS33-SystemMaster.dsk"
);

fn slots(entries: &[(u8, SlotDevice)]) -> BTreeMap<u8, SlotDevice> {
    entries.iter().copied().collect()
}

fn plus_with(entries: &[(u8, SlotDevice)]) -> Two {
    Two::new_with_slots(TwoType::Apple2Plus, None, Slot0::Language, &slots(entries))
        .expect("must construct")
}

/// Step until the predicate holds, with a cycle cap. The predicate is
/// checked every ~50K cycles — predicates like "the screen contains X"
/// are far too expensive to evaluate per instruction.
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
            cycles += two.cpu.step() as u64;
        }
    }
}

/// Type a line through the keyboard latch, waiting for the strobe to be
/// consumed after each key (the two_boot.rs/two_dos.rs pattern).
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

/// Step a fixed number of cycles.
fn step(two: &mut Two, cycles: u64) {
    let mut done = 0u64;
    while done < cycles {
        done += two.cpu.step() as u64;
    }
}

/// Call a firmware entry the way ProDOS does: JSR to it and run until the
/// RTS pops our sentinel return address.
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

// --- Multiple Disk II controllers -----------------------------------------

#[test]
fn two_controllers_have_independent_state() {
    let mut two = plus_with(&[
        (1, SlotDevice::Thunderclock),
        (5, SlotDevice::DiskII),
        (6, SlotDevice::DiskII),
    ]);

    // Both P5 boot ROMs are present at their slots' pages.
    assert_eq!(two.cpu.mem.read(0xc500), 0xa2, "slot 5 boot ROM");
    assert_eq!(two.cpu.mem.read(0xc600), 0xa2, "slot 6 boot ROM");

    // Stepping slot 5's head (phase switches at $C0D0-$C0D7) leaves slot 6
    // untouched, and vice versa ($C0E0-$C0E7).
    assert_eq!(two.cpu.mem.read(0xc0d3), 0); // slot 5 phase 1 on: +1
    assert_eq!(two.dsk_at(5).unwrap().half_track(), 1);
    assert_eq!(
        two.dsk_at(6).unwrap().half_track(),
        0,
        "slot 6 must not move"
    );
    two.cpu.mem.read(0xc0e3); // slot 6 phase 1 on: +1
    two.cpu.mem.read(0xc0e5); // slot 6 phase 2 on: +1
    assert_eq!(two.dsk_at(6).unwrap().half_track(), 2);
    assert_eq!(
        two.dsk_at(5).unwrap().half_track(),
        1,
        "slot 5 must not move"
    );

    // Motors are per-controller: turning slot 5's on lights only slot 5.
    two.cpu.mem.read(0xc0d9);
    assert!(two.dsk_at(5).unwrap().drive_lit(0, 0));
    assert!(!two.dsk_at(6).unwrap().drive_lit(0, 0));
    // The OR'ed panel lights see it.
    assert_eq!(two.drive_lights(0), [true, false]);

    // With a disk in slot 5, its data port streams nibbles; the empty slot 6
    // controller's port stays silent.
    two.load_disk_at(5, 0, DOS33).expect("load slot 5");
    two.cpu.mem.read(0xc0de); // slot 5 read mode
    let five: Vec<u8> = (0..64).map(|_| two.cpu.mem.read(0xc0dc)).collect();
    assert!(
        five.iter().any(|&b| b >= 0x96),
        "slot 5 must stream disk nibbles, got {five:02x?}"
    );
    two.cpu.mem.read(0xc0e9); // slot 6 motor on
    two.cpu.mem.read(0xc0ee); // slot 6 read mode
    let six: Vec<u8> = (0..64).map(|_| two.cpu.mem.read(0xc0ec)).collect();
    assert!(
        six.iter().all(|&b| b == 0),
        "the empty slot 6 drive must stream nothing, got {six:02x?}"
    );
}

// --- Boot-scan order --------------------------------------------------------

#[test]
fn lone_controller_in_slot_5_boots_dos() {
    let mut two = plus_with(&[(5, SlotDevice::DiskII)]);
    two.load_disk_at(5, 0, DOS33).expect("load slot 5");
    two.cpu.reset();
    step_until(&mut two, 400_000_000, "the ] prompt", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });
}

#[test]
fn autostart_scans_the_highest_slot_first() {
    // Controllers in 4 and 5, no disks: the scan reaches slot 5 first and
    // its boot ROM spins forever waiting for a disk — the PC parks in $C5xx.
    let mut two = plus_with(&[(4, SlotDevice::DiskII), (5, SlotDevice::DiskII)]);
    two.cpu.reset();
    let mut cycles = 0u64;
    while cycles < 3_000_000 {
        cycles += two.cpu.step() as u64;
    }
    assert!(
        (0xc500..=0xc5ff).contains(&two.cpu.pc),
        "expected the PC in the slot 5 boot ROM, got ${:04X}",
        two.cpu.pc
    );
}

#[test]
fn higher_slot_with_a_disk_boots_before_a_lower_one() {
    // A disk in slot 6 only: if the scan tried slot 5 first it would hang in
    // that empty drive's boot ROM, so reaching DOS proves 6 was scanned first.
    let mut two = plus_with(&[(5, SlotDevice::DiskII), (6, SlotDevice::DiskII)]);
    two.load_disk_at(6, 0, DOS33).expect("load slot 6");
    two.cpu.reset();
    step_until(&mut two, 400_000_000, "the ] prompt", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });
}

#[test]
fn three_controllers_catalog_every_drive() {
    // The full three-controller / six-drive maximum, with the DOS 3.3
    // System Master in every drive.
    let mut two = plus_with(&[
        (4, SlotDevice::DiskII),
        (5, SlotDevice::DiskII),
        (6, SlotDevice::DiskII),
    ]);
    for slot in [4, 5, 6] {
        for drive in [0, 1] {
            two.load_disk_at(slot, drive, DOS33)
                .unwrap_or_else(|e| panic!("load slot {slot} drive {drive}: {e}"));
        }
    }
    two.cpu.reset();

    // The Autostart scan boots from S6,D1 (the highest slot; drive 1 is
    // the boot drive).
    step_until(&mut two, 400_000_000, "the DOS banner", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });
    // Proof it was slot 6: the scan never ran the lower slots' boot ROMs,
    // so their heads have not moved, while slot 6 booted from drive 1.
    assert_eq!(two.dsk_at(4).unwrap().half_track(), 0, "slot 4 untouched");
    assert_eq!(two.dsk_at(5).unwrap().half_track(), 0, "slot 5 untouched");
    assert_eq!(two.dsk_at(6).unwrap().active_drive(), 0, "booted from D1");

    // CATALOG every slot/drive combination. DOS pauses a long listing when
    // the screen fills, so before each command a bare Return releases any
    // pending pause (a harmless empty line when DOS is already at the
    // prompt) and HOME clears the screen so each header check is fresh.
    for slot in [4u8, 5, 6] {
        for drive in [1u8, 2] {
            two.key(0x0d);
            step_until(&mut two, 2_000_000, "release strobe", |two| {
                two.key_register() & 0x80 == 0
            });
            step(&mut two, 8_000_000); // let a paused listing finish
            type_line(&mut two, "HOME");
            step(&mut two, 1_000_000);
            assert!(
                !two.text_screen().contains("DISK VOLUME"),
                "screen must be clear before CATALOG,S{slot},D{drive}"
            );

            type_line(&mut two, &format!("CATALOG,S{slot},D{drive}"));
            step_until(
                &mut two,
                60_000_000,
                &format!("the S{slot},D{drive} catalog"),
                |two| {
                    let text = two.text_screen();
                    text.contains("DISK VOLUME 254") && text.contains("HELLO")
                },
            );
            // The targeted controller did the work: the named drive is
            // selected with its head on the catalog track (17).
            let dsk = two.dsk_at(slot).unwrap();
            assert_eq!(dsk.active_drive(), drive as usize - 1, "S{slot},D{drive}");
            assert_eq!(dsk.half_track(), 34, "S{slot},D{drive} head on track 17");
        }
    }
}

// --- Empty slots -------------------------------------------------------------

#[test]
fn empty_slots_read_zero_and_the_machine_falls_through_to_basic() {
    let mut two = plus_with(&[(1, SlotDevice::Thunderclock)]);
    // The vacated slot 6 ROM page and DEVSEL range read $00 — which fails
    // the Autostart boot signature, so the scan skips them.
    assert_eq!(two.cpu.mem.read(0xc600), 0x00);
    assert_eq!(two.cpu.mem.read(0xc0e0), 0x00);
    two.cpu.reset();
    step_until(&mut two, 10_000_000, "the ] prompt", |two| {
        two.text_screen().contains(']')
    });
}

#[test]
fn empty_slots_read_zero_on_the_iie() {
    let mut two = Two::new_with_slots(
        TwoType::Apple2EEnhanced,
        None,
        Slot0::Language,
        &slots(&[(1, SlotDevice::Thunderclock)]),
    )
    .expect("must construct");
    assert_eq!(two.cpu.mem.read(0xc600), 0x00, "no slot 6 card ROM");
    assert_eq!(two.cpu.mem.read(0xc100), 0x08, "the slot 1 clock is there");
}

// --- A clock moved out of slot 1 ---------------------------------------------

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

#[test]
fn thunderclock_works_in_slot_2() {
    let mut two = plus_with(&[(2, SlotDevice::Thunderclock), (6, SlotDevice::DiskII)]);

    // The ProDOS clock ID bytes at the slot 2 page, and no boot signature.
    let mem = &mut two.cpu.mem;
    assert_eq!(mem.read(0xc200), 0x08);
    assert_eq!(mem.read(0xc202), 0x28);
    assert_eq!(mem.read(0xc204), 0x58);
    assert_eq!(mem.read(0xc206), 0x70);
    assert_ne!(mem.read(0xc201), 0x20, "must not look bootable");

    // The ports moved with the card: latch at $C0A0, data at $C0A1.
    two.clk_mut().set_fixed_time(sample());
    two.cpu.mem.write(0xc0a0, 0);
    for (i, want) in sample_string().iter().enumerate() {
        assert_eq!(two.cpu.mem.read(0xc0a1), *want, "byte {i}");
    }

    // The patched firmware deposits the string at $0200 from its slot 2
    // entry points, exactly as the slot 1 card does.
    for addr in 0x0200..0x0220u16 {
        two.cpu.mem.write(addr, 0xee);
    }
    call_entry(&mut two, clk_read_entry(2));
    for (i, want) in sample_string().iter().enumerate() {
        assert_eq!(two.cpu.mem.read(0x0200 + i as u16), *want, "$0200+{i}");
    }
}

#[test]
fn prodos_boot_reads_a_clock_in_slot_2() {
    // The full proof that ProDOS finds the clock by its ID bytes in any
    // slot: boot ProDOS 2.4.3 with the clock in slot 2 and wait for the
    // date/time globals. Work on a throwaway copy — ProDOS writes to the
    // image during boot.
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/../disks/ProDOS_2_4_3.po");
    let path = std::env::temp_dir().join(format!("ewm-slots-prodos-{}.po", std::process::id()));
    std::fs::copy(src, &path).expect("cannot copy ProDOS image");

    let mut two = plus_with(&[(2, SlotDevice::Thunderclock), (6, SlotDevice::DiskII)]);
    two.attach_hdd(path.to_str().unwrap()).expect("attach_hdd");
    two.clk_mut().set_fixed_time(sample());
    two.cpu.reset();

    // The packed date word for 2026-07-06 is (26<<9)|(7<<5)|6 = $34E6.
    //   $BF90 DATELO, $BF91 DATEHI, $BF92 minute, $BF93 hour
    let settled = |two: &Two| {
        let ram = two.ram();
        ram[0xbf90] == 0xe6 && ram[0xbf91] == 0x34 && ram[0xbf92] == 30 && ram[0xbf93] == 10
    };
    step_until(&mut two, 400_000_000, "the ProDOS clock globals", settled);

    std::fs::remove_file(&path).ok();
}

// --- Hard drives moved and multiplied ----------------------------------------

/// A temp image of `blocks` 512-byte blocks, block *b* filled with `b + salt`.
fn make_image(name: &str, blocks: usize, salt: u8) -> String {
    let mut image = Vec::with_capacity(blocks * 512);
    for b in 0..blocks {
        image.extend(std::iter::repeat_n(b as u8 + salt, 512));
    }
    let path = std::env::temp_dir().join(format!("ewm-slots-{name}-{}.hdv", std::process::id()));
    std::fs::write(&path, &image).expect("cannot write test image");
    path.to_str().unwrap().to_string()
}

#[test]
fn hard_drives_in_two_slots_serve_their_own_images() {
    let a = make_image("hdd5", 8, 0x50);
    let b = make_image("hdd7", 16, 0x70);
    let mut two = plus_with(&[(6, SlotDevice::DiskII)]);
    two.attach_hdd_at(5, &a).expect("attach slot 5");
    two.attach_hdd_at(7, &b).expect("attach slot 7");

    // Each card's ports serve its own image: block 2 through slot 5's ports
    // ($C0D0-$C0D6) vs slot 7's ($C0F0-$C0F6).
    let mem = &mut two.cpu.mem;
    mem.write(0xc0d0, 2);
    mem.write(0xc0d1, 0);
    assert_eq!(mem.read(0xc0d2), 0);
    assert_eq!(mem.read(0xc0d3), 0x52, "slot 5 serves image a");
    mem.write(0xc0f0, 2);
    mem.write(0xc0f1, 0);
    assert_eq!(mem.read(0xc0f2), 0);
    assert_eq!(mem.read(0xc0f3), 0x72, "slot 7 serves image b");
    // Distinct block counts through each card's STATUS ports.
    assert_eq!(mem.read(0xc0d5), 8);
    assert_eq!(mem.read(0xc0f5), 16);

    // The slot 5 firmware driver works at its own entry with unit $50.
    mem.write(0x42, 1); // READ
    mem.write(0x43, 0x50); // unit = slot 5
    mem.write(0x44, 0x00);
    mem.write(0x45, 0x10); // buffer $1000
    mem.write(0x46, 3);
    mem.write(0x47, 0); // block 3
    call_entry(&mut two, hdd_driver_entry(5));
    assert_eq!(two.cpu.a, 0, "no error");
    for addr in 0x1000..0x1200u16 {
        assert_eq!(two.cpu.mem.read(addr), 0x53, "buffer byte at {addr:04x}");
    }

    std::fs::remove_file(&a).ok();
    std::fs::remove_file(&b).ok();
}

#[test]
fn boot_scans_the_higher_hard_drive_first() {
    // Boot block only in the slot 7 image; booting it proves 7 beat 5.
    let mut block0 = [0u8; 512];
    block0[0] = 0x01;
    let code: [u8; 16] = [
        0xa2, 0x00, 0xbd, 0x20, 0x08, 0xf0, 0x06, 0x9d, 0x00, 0x04, 0xe8, 0xd0, 0xf5, 0x4c, 0x0e,
        0x08,
    ];
    block0[1..1 + code.len()].copy_from_slice(&code);
    for (i, b) in b"HDD BOOT OK".iter().enumerate() {
        block0[0x20 + i] = b | 0x80;
    }
    let path = std::env::temp_dir().join(format!("ewm-slots-hddboot-{}.hdv", std::process::id()));
    let mut image = block0.to_vec();
    image.extend(std::iter::repeat_n(0u8, 512));
    std::fs::write(&path, &image).unwrap();
    let plain = make_image("hddplain", 4, 0);

    let mut two = plus_with(&[(6, SlotDevice::DiskII)]);
    two.attach_hdd_at(5, &plain).expect("attach slot 5");
    two.attach_hdd_at(7, path.to_str().unwrap())
        .expect("attach slot 7");
    two.cpu.reset();
    step_until(&mut two, 10_000_000, "the slot 7 boot block", |two| {
        two.text_screen().contains("HDD BOOT OK")
    });

    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&plain).ok();
}

// --- Slot 0: the language card ------------------------------------------------

#[test]
fn slot_zero_language_card_is_optional() {
    // With the card (the classic 64K build): two reads of $C083 read- and
    // write-enable bank 2 RAM at $D000, so a write sticks.
    let mut two = Two::new_with_slots(TwoType::Apple2Plus, None, Slot0::Language, &slots(&[]))
        .expect("must construct");
    assert_eq!(two.slot0(), Slot0::Language);
    let rom_byte = two.cpu.mem.read(0xd000);
    two.cpu.mem.read(0xc083);
    two.cpu.mem.read(0xc083);
    two.cpu.mem.write(0xd000, rom_byte.wrapping_add(1));
    assert_eq!(
        two.cpu.mem.read(0xd000),
        rom_byte.wrapping_add(1),
        "language-card RAM must take the write"
    );

    // Without it (the 48K machine): $D000-$FFFF is motherboard ROM straight
    // on the bus, the same switch sequence changes nothing, and slot 0's
    // DEVSEL range is as unmapped as any other empty slot's.
    let mut two = Two::new_with_slots(TwoType::Apple2Plus, None, Slot0::Empty, &slots(&[]))
        .expect("must construct");
    assert_eq!(two.slot0(), Slot0::Empty);
    assert_eq!(two.cpu.mem.read(0xd000), rom_byte, "the same machine ROM");
    assert_eq!(two.cpu.mem.read(0xc083), 0x00, "slot 0 DEVSEL is unmapped");
    two.cpu.mem.read(0xc083);
    two.cpu.mem.write(0xd000, rom_byte.wrapping_add(1));
    assert_eq!(two.cpu.mem.read(0xd000), rom_byte, "ROM must stay ROM");
}

#[test]
fn dos33_boots_on_a_48k_machine() {
    // DOS 3.3 probes for the language card and just skips loading Integer
    // BASIC into it — the 48K machine still boots to the prompt.
    let mut two = Two::new_with_slots(
        TwoType::Apple2Plus,
        None,
        Slot0::Empty,
        &slots(&[(6, SlotDevice::DiskII)]),
    )
    .expect("must construct");
    two.load_disk_at(6, 0, DOS33).expect("load slot 6");
    two.cpu.reset();
    step_until(&mut two, 400_000_000, "the ] prompt", |two| {
        let text = two.text_screen();
        text.contains("DOS VERSION 3.3") && text.contains(']')
    });
}

#[test]
fn construction_rejects_bad_tables_and_occupied_slots() {
    let err = Two::new_with_slots(
        TwoType::Apple2Plus,
        None,
        Slot0::Language,
        &slots(&[(8, SlotDevice::DiskII)]),
    )
    .err()
    .expect("slot 8 must be rejected");
    assert!(err.contains("no such slot 8"), "{err}");

    let err = Two::new_with_slots(
        TwoType::Apple2Plus,
        None,
        Slot0::Language,
        &slots(&[(1, SlotDevice::Thunderclock), (2, SlotDevice::Thunderclock)]),
    )
    .err()
    .expect("two clocks must be rejected");
    assert!(err.contains("at most one Thunderclock"), "{err}");

    let image = make_image("occupied", 4, 0);
    let mut two = Two::new(TwoType::Apple2Plus).expect("must construct");
    let err = two.attach_hdd_at(6, &image).unwrap_err();
    assert!(err.contains("already occupied"), "{err}");
    std::fs::remove_file(&image).ok();
}
