//! Slot 7 hard drive tests: the card's port protocol, write persistence to
//! the image file, the hand-assembled firmware driver, and a full boot from
//! a synthetic ProDOS block image via the Autostart slot scan.

use ewm::hdd::HDD_DRIVER_ENTRY;
use ewm::two::{Two, TwoType};

/// A temp image of `blocks` 512-byte blocks, block *b* filled with byte *b*.
fn make_image(name: &str, blocks: usize) -> String {
    let mut image = Vec::with_capacity(blocks * 512);
    for b in 0..blocks {
        image.extend(std::iter::repeat_n(b as u8, 512));
    }
    let path = std::env::temp_dir().join(format!("ewm-hdd-test-{name}-{}.hdv", std::process::id()));
    std::fs::write(&path, &image).expect("cannot write test image");
    path.to_str().unwrap().to_string()
}

fn machine_with_image(path: &str) -> Two {
    let mut two = Two::new(TwoType::Apple2Plus).expect("apple2plus must construct");
    two.attach_hdd(path).expect("attach_hdd failed");
    two
}

#[test]
fn ports_read_a_block() {
    let path = make_image("ports", 16);
    let mut two = machine_with_image(&path);
    let mem = &mut two.cpu.mem;

    // Select block 5 and execute a read.
    mem.write(0xc0f0, 5);
    mem.write(0xc0f1, 0);
    assert_eq!(mem.read(0xc0f2), 0, "read of a valid block must succeed");
    for i in 0..512 {
        assert_eq!(mem.read(0xc0f3), 5, "byte {i} of block 5");
    }

    // Block count ports.
    assert_eq!(mem.read(0xc0f5), 16);
    assert_eq!(mem.read(0xc0f6), 0);

    // Out of range block: ProDOS I/O error.
    mem.write(0xc0f0, 16);
    assert_eq!(mem.read(0xc0f2), 0x27);

    std::fs::remove_file(&path).ok();
}

#[test]
fn writes_persist_to_the_image_file() {
    let path = make_image("persist", 16);
    let mut two = machine_with_image(&path);
    let mem = &mut two.cpu.mem;

    // Write a recognizable pattern into block 7 and commit.
    mem.write(0xc0f0, 7);
    mem.write(0xc0f1, 0);
    for i in 0..512u16 {
        mem.write(0xc0f3, (i % 251) as u8);
    }
    assert_eq!(mem.read(0xc0f4), 0, "commit must succeed");

    // Read back through the card...
    mem.write(0xc0f0, 7);
    assert_eq!(mem.read(0xc0f2), 0);
    for i in 0..512u16 {
        assert_eq!(mem.read(0xc0f3), (i % 251) as u8);
    }

    // ...and, the point of persistence, from the file itself.
    let file = std::fs::read(&path).unwrap();
    for i in 0..512usize {
        assert_eq!(file[7 * 512 + i], (i % 251) as u8, "file byte {i}");
    }

    std::fs::remove_file(&path).ok();
}

/// Call the firmware driver the way ProDOS does: command block in $42-$47,
/// JSR to the entry point published by the card ROM.
fn call_driver(two: &mut Two, cmd: u8, buffer: u16, block: u16) {
    let mem = &mut two.cpu.mem;
    mem.write(0x42, cmd);
    mem.write(0x43, 0x70); // unit: slot 7
    mem.write(0x44, buffer as u8);
    mem.write(0x45, (buffer >> 8) as u8);
    mem.write(0x46, block as u8);
    mem.write(0x47, (block >> 8) as u8);

    // Simulate JSR $C7xx from $1233: an RTS returns to $1234.
    two.cpu.sp = 0xff;
    two.cpu.push_word(0x1233);
    two.cpu.pc = HDD_DRIVER_ENTRY;
    let mut steps = 0;
    while two.cpu.pc != 0x1234 {
        two.cpu.step();
        steps += 1;
        assert!(steps < 100_000, "driver did not return");
    }
}

#[test]
fn firmware_driver_reads_a_block_to_the_buffer() {
    let path = make_image("driver", 16);
    let mut two = machine_with_image(&path);

    call_driver(&mut two, 1, 0x1000, 9); // READ block 9 to $1000
    assert_eq!(two.cpu.a, 0, "A = no error");
    assert_eq!(two.cpu.c, 0, "carry clear on success");
    for addr in 0x1000..0x1200u16 {
        assert_eq!(two.cpu.mem.read(addr), 9, "buffer byte at {addr:04x}");
    }
    // The driver restores the buffer pointer it increments while pumping.
    assert_eq!(two.cpu.mem.read(0x45), 0x10);

    std::fs::remove_file(&path).ok();
}

#[test]
fn firmware_driver_writes_a_block_from_the_buffer() {
    let path = make_image("driver-write", 16);
    let mut two = machine_with_image(&path);

    for addr in 0x2000..0x2200u16 {
        two.cpu.mem.write(addr, 0xa5);
    }
    call_driver(&mut two, 2, 0x2000, 3); // WRITE $2000 to block 3
    assert_eq!(two.cpu.a, 0);
    assert_eq!(two.cpu.c, 0);

    let file = std::fs::read(&path).unwrap();
    assert!(file[3 * 512..4 * 512].iter().all(|&b| b == 0xa5));

    std::fs::remove_file(&path).ok();
}

#[test]
fn firmware_driver_status_returns_block_count() {
    let path = make_image("status", 16);
    let mut two = machine_with_image(&path);

    call_driver(&mut two, 0, 0, 0); // STATUS
    assert_eq!(two.cpu.a, 0);
    assert_eq!(two.cpu.c, 0);
    assert_eq!(two.cpu.x, 16, "X = block count low");
    assert_eq!(two.cpu.y, 0, "Y = block count high");

    std::fs::remove_file(&path).ok();
}

#[test]
fn machine_boots_from_the_hard_drive() {
    // Block 0 is a ProDOS-convention boot block: loaded at $0800 and entered
    // at $0801. This one writes "HDD BOOT OK" to text page 1 and hangs.
    //
    //   0801: A2 00     LDX #$00
    //   0803: BD 20 08  LDA $0820,X
    //   0806: F0 06     BEQ $080E
    //   0808: 9D 00 04  STA $0400,X
    //   080B: E8        INX
    //   080C: D0 F5     BNE $0803
    //   080E: 4C 0E 08  JMP $080E
    let mut block0 = [0u8; 512];
    block0[0] = 0x01;
    let code: [u8; 16] = [
        0xa2, 0x00, 0xbd, 0x20, 0x08, 0xf0, 0x06, 0x9d, 0x00, 0x04, 0xe8, 0xd0, 0xf5, 0x4c, 0x0e,
        0x08,
    ];
    block0[1..1 + code.len()].copy_from_slice(&code);
    for (i, b) in b"HDD BOOT OK".iter().enumerate() {
        block0[0x20 + i] = b | 0x80; // normal screen codes
    }

    let path = std::env::temp_dir().join(format!("ewm-hdd-test-boot-{}.hdv", std::process::id()));
    let mut image = block0.to_vec();
    image.extend(std::iter::repeat_n(0u8, 512)); // block 1, to be plausible
    std::fs::write(&path, &image).unwrap();

    // No floppy: the Autostart slot scan must find the slot 7 card first.
    let mut two = machine_with_image(path.to_str().unwrap());
    two.cpu.reset();
    let mut cycles = 0u64;
    while !two.text_screen().contains("HDD BOOT OK") {
        cycles += two.cpu.step() as u64;
        assert!(
            cycles < 10_000_000,
            "did not boot from the hard drive; screen was:\n{}",
            two.text_screen()
        );
    }

    std::fs::remove_file(&path).ok();
}
