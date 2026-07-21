//! The UniDisk 3.5 Controller (Liron): the hand-assembled firmware's ProDOS
//! block driver and SmartPort dispatch called the way real software calls
//! them, write-back into the .2mg container, and an end-to-end ProDOS boot
//! from an 800K image via the Autostart slot scan.

use std::collections::BTreeMap;

use ewm::liron::{liron_prodos_entry, liron_smartport_entry};
use ewm::two::{Slot0, SlotDevice, Two, TwoType};

/// A .2mg of `blocks` 512-byte blocks whose data payload is `data` padded
/// with zeros (or, when `data` is empty, block *b* filled with byte *b*).
fn make_2mg(name: &str, blocks: usize, data: &[u8]) -> String {
    let mut raw = vec![0u8; 64];
    raw[0..4].copy_from_slice(b"2IMG");
    raw[4..8].copy_from_slice(b"EWM!");
    raw[8..10].copy_from_slice(&64u16.to_le_bytes());
    raw[10..12].copy_from_slice(&1u16.to_le_bytes());
    raw[0x0c..0x10].copy_from_slice(&1u32.to_le_bytes()); // ProDOS order
    raw[0x14..0x18].copy_from_slice(&(blocks as u32).to_le_bytes());
    raw[0x18..0x1c].copy_from_slice(&64u32.to_le_bytes());
    raw[0x1c..0x20].copy_from_slice(&(blocks as u32 * 512).to_le_bytes());
    if data.is_empty() {
        for b in 0..blocks {
            raw.extend(std::iter::repeat_n(b as u8, 512));
        }
    } else {
        assert!(data.len() <= blocks * 512);
        raw.extend_from_slice(data);
        raw.resize(64 + blocks * 512, 0);
    }
    let path = std::env::temp_dir().join(format!("ewm-liron-it-{name}-{}.2mg", std::process::id()));
    std::fs::write(&path, &raw).expect("cannot write test image");
    path.to_str().unwrap().to_string()
}

/// A ][+ with a Liron in slot 5 (the card predates the //e's launch and
/// works in both machines; the //e gets the boot test below).
fn machine() -> Two {
    let slots: BTreeMap<u8, SlotDevice> = [(5, SlotDevice::Liron)].into();
    Two::new_with_slots(TwoType::Apple2Plus, None, Slot0::Language, &slots).expect("must construct")
}

fn run_until(two: &mut Two, target: u16) {
    let mut steps = 0;
    while two.cpu.pc != target {
        two.cpu.step();
        steps += 1;
        assert!(steps < 100_000, "firmware did not return");
    }
}

/// Call the ProDOS entry the way ProDOS does: command block in $42-$47,
/// JSR to the entry the ROM publishes at $CnFF.
fn call_prodos(two: &mut Two, cmd: u8, unit: u8, buffer: u16, block: u16) {
    let mem = &mut two.cpu.mem;
    mem.write(0x42, cmd);
    mem.write(0x43, unit);
    mem.write(0x44, buffer as u8);
    mem.write(0x45, (buffer >> 8) as u8);
    mem.write(0x46, block as u8);
    mem.write(0x47, (block >> 8) as u8);
    two.cpu.sp = 0xff;
    two.cpu.push_word(0x1233);
    two.cpu.pc = liron_prodos_entry(5);
    run_until(two, 0x1234);
}

/// Issue a SmartPort call exactly as the convention demands: a JSR to the
/// dispatch address followed by an inline command byte and parameter-list
/// pointer, resuming past the inline bytes.
fn call_smartport(two: &mut Two, cmd: u8, list: &[u8]) {
    let mem = &mut two.cpu.mem;
    for (i, &b) in list.iter().enumerate() {
        mem.write(0x2000 + i as u16, b);
    }
    let entry = liron_smartport_entry(5);
    mem.write(0x1000, 0x20); // JSR dispatch
    mem.write(0x1001, entry as u8);
    mem.write(0x1002, (entry >> 8) as u8);
    mem.write(0x1003, cmd);
    mem.write(0x1004, 0x00); // list at $2000
    mem.write(0x1005, 0x20);
    two.cpu.sp = 0xff;
    two.cpu.pc = 0x1000;
    run_until(two, 0x1006);
}

#[test]
fn prodos_entry_reads_writes_and_statuses_both_drives() {
    let p1 = make_2mg("prodos-d1", 800, &[]);
    let p2 = make_2mg("prodos-d2", 1600, &[]);
    let mut two = machine();
    two.load_2mg_at(5, 0, &p1).expect("mount drive 1");
    two.load_2mg_at(5, 1, &p2).expect("mount drive 2");

    // STATUS: block counts per drive in X/Y.
    call_prodos(&mut two, 0, 0x50, 0, 0);
    assert_eq!(two.cpu.c, 0);
    assert_eq!(two.cpu.x as u16 | ((two.cpu.y as u16) << 8), 800);
    call_prodos(&mut two, 0, 0xd0, 0, 0);
    assert_eq!(two.cpu.x as u16 | ((two.cpu.y as u16) << 8), 1600);

    // READ block 9 of drive 1 and block 900 of drive 2 (drive 1 is too
    // small for 900, proving the drive bit routes).
    call_prodos(&mut two, 1, 0x50, 0x1000, 9);
    assert_eq!(two.cpu.a, 0, "drive 1 read error: ${:02x}", two.cpu.a);
    assert_eq!(two.cpu.c, 0);
    assert_eq!(two.cpu.mem.read(0x1000), 9);
    assert_eq!(two.cpu.mem.read(0x11ff), 9);
    call_prodos(&mut two, 1, 0xd0, 0x1000, 900);
    assert_eq!(two.cpu.a, 0);
    assert_eq!(two.cpu.mem.read(0x1000), (900 % 256) as u8);
    call_prodos(&mut two, 1, 0x50, 0x1000, 900);
    assert_eq!(two.cpu.c, 1, "block 900 of the 400K drive must fail");
    assert_eq!(two.cpu.a, 0x27, "with a ProDOS I/O error");

    // WRITE persists into the .2mg past the header.
    for addr in 0x3000..0x3200u16 {
        two.cpu.mem.write(addr, 0x5a);
    }
    call_prodos(&mut two, 2, 0x50, 0x3000, 11);
    assert_eq!(two.cpu.a, 0);
    let raw = std::fs::read(&p1).unwrap();
    assert!(raw[64 + 11 * 512..64 + 12 * 512].iter().all(|&b| b == 0x5a));
    assert_eq!(raw[64 + 10 * 512], 10, "the neighboring block is intact");

    // An empty drive is off-line at read time.
    let mut empty = machine();
    call_prodos(&mut empty, 1, 0x50, 0x1000, 0);
    assert_eq!(empty.cpu.c, 1);
    assert_eq!(empty.cpu.a, 0x2f, "DEVICE OFF-LINE");

    std::fs::remove_file(&p1).ok();
    std::fs::remove_file(&p2).ok();
}

#[test]
fn smartport_status_and_block_calls() {
    let path = make_2mg("sp", 1600, &[]);
    let mut two = machine();
    two.load_2mg_at(5, 0, &path).expect("mount drive 1");

    // STATUS unit 0 code 0: the controller reports two devices.
    call_smartport(&mut two, 0, &[3, 0, 0x00, 0x30, 0]); // status list $3000
    assert_eq!(two.cpu.c, 0, "controller status must succeed");
    assert_eq!(two.cpu.mem.read(0x3000), 2, "device count");

    // STATUS unit 1 code 0: online with 1600 blocks, little-endian.
    call_smartport(&mut two, 0, &[3, 1, 0x00, 0x30, 0]);
    assert_eq!(two.cpu.c, 0);
    let status = two.cpu.mem.read(0x3000);
    assert_eq!(status & 0b1001_0000, 0b1001_0000, "block device, online");
    assert_eq!(two.cpu.mem.read(0x3001), (1600u16 & 0xff) as u8);
    assert_eq!(two.cpu.mem.read(0x3002), (1600u16 >> 8) as u8);
    assert_eq!(two.cpu.mem.read(0x3003), 0);

    // READ_BLOCK unit 1, block 7, into $4000.
    call_smartport(&mut two, 1, &[3, 1, 0x00, 0x40, 7, 0, 0]);
    assert_eq!(two.cpu.a, 0, "SP read error: ${:02x}", two.cpu.a);
    assert_eq!(two.cpu.c, 0);
    assert_eq!(two.cpu.mem.read(0x4000), 7);
    assert_eq!(two.cpu.mem.read(0x41ff), 7);

    // WRITE_BLOCK unit 1, block 13, from $4000.
    for addr in 0x4000..0x4200u16 {
        two.cpu.mem.write(addr, 0xc3);
    }
    call_smartport(&mut two, 2, &[3, 1, 0x00, 0x40, 13, 0, 0]);
    assert_eq!(two.cpu.c, 0);
    let raw = std::fs::read(&path).unwrap();
    assert!(raw[64 + 13 * 512..64 + 14 * 512].iter().all(|&b| b == 0xc3));

    // An unimplemented command errors with BADCMD.
    call_smartport(&mut two, 4, &[1, 1]); // CONTROL
    assert_eq!(two.cpu.c, 1);
    assert_eq!(two.cpu.a, 0x21);

    std::fs::remove_file(&path).ok();
}

#[test]
fn prodos_boots_from_an_800k_liron_image_on_the_iie() {
    // ProDOS 2.4.3's 280 blocks padded into an 800K .2mg: the volume only
    // claims its 280 blocks, which is fine — what matters is booting
    // through the Liron's firmware via the Autostart slot scan (which must
    // accept the SmartPort signature, $Cn07=$00).
    let prodos = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../disks/ProDOS_2_4_3.po"
    ))
    .expect("cannot read ProDOS_2_4_3.po");
    let path = make_2mg("boot", 1600, &prodos);

    let slots: BTreeMap<u8, SlotDevice> =
        [(1, SlotDevice::Thunderclock), (5, SlotDevice::Liron)].into();
    let mut two = Two::new_with_slots(TwoType::Apple2EEnhanced, None, Slot0::Language, &slots)
        .expect("must construct");
    two.load_2mg_at(5, 0, &path).expect("mount the boot image");
    two.cpu.reset();

    let mut spent = 0u64;
    while !two.text_screen().contains("PRODOS.2.4.3") {
        let mut done = 0u64;
        while done < 10_000_000 {
            done += two.cpu.step() as u64;
        }
        spent += 10_000_000;
        assert!(
            spent < 250_000_000,
            "ProDOS did not boot from the Liron; screen was:\n{}",
            two.text_screen()
        );
    }
    assert!(
        two.text_screen().contains("BITSY.BOOT"),
        "expected the Bitsy Bye listing; screen was:\n{}",
        two.text_screen()
    );

    std::fs::remove_file(&path).ok();
}
