//! Hard disk controller for ProDOS block images (.hdv/.po), the device that
//! boots Total Replay — slot 7 by default, usable in any slot. Like every
//! emulator's hard disk card this is virtual hardware: a 256-byte firmware
//! ROM at $Cn00 speaks the ProDOS block-driver protocol ($42-$47) and pumps
//! data byte-by-byte through magic I/O ports in the slot's DEVSEL range
//! ($C080 + slot*16), which this device serves from the image. All data
//! moves over the normal bus — the card never touches machine RAM directly.
//!
//! The Autostart ROM's slot scan (7 down to 1) boots the first slot whose
//! ROM has $Cn01=$20, $Cn03=$00, $Cn05=$03 and $Cn07=$3C, so a drive in
//! slot 7 boots before a Disk II in slot 6.
//!
//! Ports (shown for slot 7; the low nibble is what the device decodes):
//!   $C0F0 W  block number low
//!   $C0F1 W  block number high     (writing either resets the data index)
//!   $C0F2 R  execute READ: block -> card buffer; returns ProDOS error code
//!   $C0F3 RW data port, auto-incrementing through the 512-byte buffer
//!   $C0F4 R  execute WRITE: buffer -> image + file; returns error code
//!   $C0F5 R  total blocks, low byte
//!   $C0F6 R  total blocks, high byte

use std::io::{Seek, SeekFrom, Write};

use ewm_core::mem::Device;

// ProDOS MLI error codes returned by the driver.
const ERR_IO: u8 = 0x27;
const ERR_WRITE_PROTECTED: u8 = 0x2b;

// The firmware for a card in slot `n`. Hand-assembled; the listing below is
// the source of truth for the bytes (shown for slot 7 — the unit byte, the
// $Cn page and the $C0xx port operands are computed from `slot`). The boot
// half follows the ProDOS convention (block 0 to $0800, jump $0801 with
// X = slot*16); the driver half implements STATUS/READ/WRITE/FORMAT from
// $42-$47 and is published via $CnFF.
//
//   ; boot signature: the LDA-immediate trick puts $20/$00/$03/$3C at
//   ; offsets 1/3/5/7, which is what the Autostart slot scan checks.
//   C700: A9 20     LDA #$20
//   C702: A9 00     LDA #$00
//   C704: A9 03     LDA #$03
//   C706: A9 3C     LDA #$3C
//   ; boot: READ block 0 to $0800
//   C708: A9 01     LDA #$01
//   C70A: 85 42     STA $42        ; command = READ
//   C70C: A9 70     LDA #$70
//   C70E: 85 43     STA $43        ; unit = slot 7
//   C710: A9 00     LDA #$00
//   C712: 85 44     STA $44        ; buffer = $0800
//   C714: 85 46     STA $46        ; block = 0
//   C716: 85 47     STA $47
//   C718: A9 08     LDA #$08
//   C71A: 85 45     STA $45
//   C71C: 20 40 C7  JSR $C740      ; call the driver
//   C71F: B0 05     BCS $C726      ; boot failed
//   C721: A2 70     LDX #$70       ; X = slot*16, as boot blocks expect
//   C723: 4C 01 08  JMP $0801
//   C726: 4C 00 E0  JMP $E000      ; fall into BASIC
//   ; driver entry, published at $C7FF
//   C740: A5 42     LDA $42
//   C742: F0 54     BEQ $C798      ; 0 = STATUS
//   C744: C9 01     CMP #$01
//   C746: F0 08     BEQ $C750      ; 1 = READ
//   C748: C9 02     CMP #$02
//   C74A: F0 28     BEQ $C774      ; 2 = WRITE
//   C74C: A9 00     LDA #$00       ; 3 = FORMAT: succeed as a no-op
//   C74E: 18        CLC
//   C74F: 60        RTS
//   ; READ: execute, then pump 2 pages from the data port to ($44)
//   C750: 20 A2 C7  JSR $C7A2      ; set block ports
//   C753: AD F2 C0  LDA $C0F2      ; execute read, A = error code
//   C756: D0 3E     BNE $C796
//   C758: A2 02     LDX #$02
//   C75A: A0 00     LDY #$00
//   C75C: AD F3 C0  LDA $C0F3
//   C75F: 91 44     STA ($44),Y
//   C761: C8        INY
//   C762: D0 F8     BNE $C75C
//   C764: E6 45     INC $45
//   C766: CA        DEX
//   C767: D0 F3     BNE $C75C
//   C769: A5 45     LDA $45        ; restore the buffer pointer
//   C76B: 38        SEC
//   C76C: E9 02     SBC #$02
//   C76E: 85 45     STA $45
//   C770: A9 00     LDA #$00
//   C772: 18        CLC
//   C773: 60        RTS
//   ; WRITE: pump 2 pages from ($44) to the data port, then commit
//   C774: 20 A2 C7  JSR $C7A2
//   C777: A2 02     LDX #$02
//   C779: A0 00     LDY #$00
//   C77B: B1 44     LDA ($44),Y
//   C77D: 8D F3 C0  STA $C0F3
//   C780: C8        INY
//   C781: D0 F8     BNE $C77B
//   C783: E6 45     INC $45
//   C785: CA        DEX
//   C786: D0 F3     BNE $C77B
//   C788: A5 45     LDA $45
//   C78A: 38        SEC
//   C78B: E9 02     SBC #$02
//   C78D: 85 45     STA $45
//   C78F: AD F4 C0  LDA $C0F4      ; commit, A = error code
//   C792: D0 02     BNE $C796
//   C794: 18        CLC
//   C795: 60        RTS
//   C796: 38        SEC            ; shared error exit, code in A
//   C797: 60        RTS
//   ; STATUS: block count in X (low) / Y (high)
//   C798: AE F5 C0  LDX $C0F5
//   C79B: AC F6 C0  LDY $C0F6
//   C79E: A9 00     LDA #$00
//   C7A0: 18        CLC
//   C7A1: 60        RTS
//   ; set the block-number ports from $46/$47
//   C7A2: A5 46     LDA $46
//   C7A4: 8D F0 C0  STA $C0F0
//   C7A7: A5 47     LDA $47
//   C7A9: 8D F1 C0  STA $C0F1
//   C7AC: 60        RTS
//   ; ProDOS ID bytes
//   C7FC: 00 00     ; block count 0 = "ask the STATUS call"
//   C7FE: 4F        ; supports status/read/write/format
//   C7FF: 40        ; driver entry offset
pub fn hdd_rom(slot: u8) -> [u8; 256] {
    let unit = slot << 4; // ProDOS unit number in $43, and X at boot
    let n = 0xc0 + slot; // the $Cn page the ROM lives in
    let p = |k: u8| 0x80 + slot * 16 + k; // low byte of DEVSEL port k
    let mut rom = [0u8; 256];
    let code: [u8; 0xad] = [
        0xa9,
        0x20,
        0xa9,
        0x00,
        0xa9,
        0x03,
        0xa9,
        0x3c, // boot signature
        0xa9,
        0x01,
        0x85,
        0x42, // command = READ
        0xa9,
        unit,
        0x85,
        0x43, // unit = slot*16
        0xa9,
        0x00,
        0x85,
        0x44,
        0x85,
        0x46,
        0x85,
        0x47, // buffer lo, block = 0
        0xa9,
        0x08,
        0x85,
        0x45, // buffer = $0800
        0x20,
        0x40,
        n, // JSR $Cn40 (the driver)
        0xb0,
        0x05, // BCS: boot failed
        0xa2,
        unit, // X = slot*16, as boot blocks expect
        0x4c,
        0x01,
        0x08, // JMP $0801
        0x4c,
        0x00,
        0xe0, // JMP $E000: fall into BASIC
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00, // pad to $Cn40
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00, //
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00, //
        // driver entry at $Cn40, published via $CnFF
        0xa5,
        0x42,
        0xf0,
        0x54, // 0 = STATUS
        0xc9,
        0x01,
        0xf0,
        0x08, // 1 = READ
        0xc9,
        0x02,
        0xf0,
        0x28, // 2 = WRITE
        0xa9,
        0x00,
        0x18,
        0x60, // 3 = FORMAT: succeed as a no-op
        // READ: execute, then pump 2 pages from the data port to ($44)
        0x20,
        0xa2,
        n, // JSR $CnA2 (set block ports)
        0xad,
        p(2),
        0xc0, // LDA $C0x2: execute read, A = error code
        0xd0,
        0x3e, // BNE error exit
        0xa2,
        0x02,
        0xa0,
        0x00, // two pages
        0xad,
        p(3),
        0xc0, // LDA $C0x3: data port
        0x91,
        0x44,
        0xc8,
        0xd0,
        0xf8, // STA ($44),Y / INY / loop
        0xe6,
        0x45,
        0xca,
        0xd0,
        0xf3, // INC $45 / DEX / loop
        0xa5,
        0x45,
        0x38,
        0xe9,
        0x02,
        0x85,
        0x45, // restore buffer pointer
        0xa9,
        0x00,
        0x18,
        0x60, // success
        // WRITE: pump 2 pages from ($44) to the data port, then commit
        0x20,
        0xa2,
        n, // JSR $CnA2
        0xa2,
        0x02,
        0xa0,
        0x00, // two pages
        0xb1,
        0x44, // LDA ($44),Y
        0x8d,
        p(3),
        0xc0, // STA $C0x3: data port
        0xc8,
        0xd0,
        0xf8, // INY / loop
        0xe6,
        0x45,
        0xca,
        0xd0,
        0xf3, // INC $45 / DEX / loop
        0xa5,
        0x45,
        0x38,
        0xe9,
        0x02,
        0x85,
        0x45, // restore buffer pointer
        0xad,
        p(4),
        0xc0, // LDA $C0x4: commit, A = error code
        0xd0,
        0x02,
        0x18,
        0x60, // success
        0x38,
        0x60, // shared error exit, code in A
        // STATUS: block count in X (low) / Y (high)
        0xae,
        p(5),
        0xc0, // LDX $C0x5
        0xac,
        p(6),
        0xc0, // LDY $C0x6
        0xa9,
        0x00,
        0x18,
        0x60, // success
        // set the block-number ports from $46/$47
        0xa5,
        0x46, //
        0x8d,
        p(0),
        0xc0, // STA $C0x0
        0xa5,
        0x47, //
        0x8d,
        p(1),
        0xc0, // STA $C0x1
        0x60, // RTS
    ];
    rom[..code.len()].copy_from_slice(&code);
    // ProDOS ID bytes: block count 0 = "ask STATUS"; supports
    // status/read/write/format; driver entry offset $40.
    rom[0xfe] = 0x4f;
    rom[0xff] = 0x40;
    rom
}

/// The driver entry point within the slot ROM ($Cn00 + the offset the ROM
/// publishes at $CnFF).
pub const fn hdd_driver_entry(slot: u8) -> u16 {
    ((0xc0 + slot as u16) << 8) | 0x40
}

/// The driver entry point for the default slot-7 card.
pub const HDD_DRIVER_ENTRY: u16 = hdd_driver_entry(7);

pub struct Hdd {
    image: Vec<u8>,
    /// The image file, kept open for write-back; `None` when it could not
    /// be opened writable — writes then fail with WRITE PROTECTED.
    file: Option<std::fs::File>,
    block: u16,
    buf: [u8; 512],
    index: usize,
}

impl Hdd {
    /// Mount a raw ProDOS block image. The file must be a whole number of
    /// 512-byte blocks, at most 65,535 of them (the ProDOS maximum — Total
    /// Replay's 33,553,920 bytes is exactly that).
    pub fn new(path: &str) -> Result<Hdd, String> {
        let image = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        if image.is_empty() || !image.len().is_multiple_of(512) {
            return Err(format!(
                "{path}: not a raw block image (size {} is not a multiple of 512)",
                image.len()
            ));
        }
        if image.len() / 512 > 0xffff {
            return Err(format!(
                "{path}: too large ({} blocks; ProDOS allows 65535)",
                image.len() / 512
            ));
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .ok();
        if file.is_none() {
            eprintln!("[HDD] {path} is not writable; mounting read-only");
        }
        Ok(Hdd {
            image,
            file,
            block: 0,
            buf: [0; 512],
            index: 0,
        })
    }

    pub fn blocks(&self) -> u16 {
        (self.image.len() / 512) as u16
    }

    fn execute_read(&mut self) -> u8 {
        self.index = 0;
        if self.block >= self.blocks() {
            return ERR_IO;
        }
        let off = self.block as usize * 512;
        self.buf.copy_from_slice(&self.image[off..off + 512]);
        0
    }

    /// Commit the buffered block to the in-memory image and the file, so
    /// saves (Total Replay preferences, high scores) persist.
    fn execute_write(&mut self) -> u8 {
        self.index = 0;
        if self.block >= self.blocks() {
            return ERR_IO;
        }
        let Some(file) = &mut self.file else {
            return ERR_WRITE_PROTECTED;
        };
        let off = self.block as usize * 512;
        self.image[off..off + 512].copy_from_slice(&self.buf);
        let written = file
            .seek(SeekFrom::Start(off as u64))
            .and_then(|_| file.write_all(&self.buf))
            .and_then(|_| file.flush());
        match written {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("[HDD] write of block {} failed: {e}", self.block);
                ERR_IO
            }
        }
    }
}

/// The card as an IO device over its slot's 16-byte DEVSEL range; only the
/// low nibble is decoded, so the same device works in any slot.
impl Device for Hdd {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        match addr & 0x0f {
            0x2 => self.execute_read(),
            0x3 => {
                let b = self.buf[self.index % 512];
                self.index = (self.index + 1) % 512;
                b
            }
            0x4 => self.execute_write(),
            0x5 => (self.blocks() & 0xff) as u8,
            0x6 => (self.blocks() >> 8) as u8,
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, b: u8, _cycles: u64) {
        match addr & 0x0f {
            0x0 => {
                self.block = (self.block & 0xff00) | b as u16;
                self.index = 0;
            }
            0x1 => {
                self.block = (self.block & 0x00ff) | ((b as u16) << 8);
                self.index = 0;
            }
            0x3 => {
                self.buf[self.index % 512] = b;
                self.index = (self.index + 1) % 512;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator reproduces the original hand-assembled slot-7 firmware
    /// byte-for-byte (the literal below is the pre-generator static).
    #[test]
    fn slot_7_rom_matches_the_original_bytes() {
        let golden: [u8; 0xad] = [
            0xa9, 0x20, 0xa9, 0x00, 0xa9, 0x03, 0xa9, 0x3c, 0xa9, 0x01, 0x85, 0x42, 0xa9, 0x70,
            0x85, 0x43, 0xa9, 0x00, 0x85, 0x44, 0x85, 0x46, 0x85, 0x47, 0xa9, 0x08, 0x85, 0x45,
            0x20, 0x40, 0xc7, 0xb0, 0x05, 0xa2, 0x70, 0x4c, 0x01, 0x08, 0x4c, 0x00, 0xe0, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa5, 0x42, 0xf0, 0x54, 0xc9, 0x01,
            0xf0, 0x08, 0xc9, 0x02, 0xf0, 0x28, 0xa9, 0x00, 0x18, 0x60, 0x20, 0xa2, 0xc7, 0xad,
            0xf2, 0xc0, 0xd0, 0x3e, 0xa2, 0x02, 0xa0, 0x00, 0xad, 0xf3, 0xc0, 0x91, 0x44, 0xc8,
            0xd0, 0xf8, 0xe6, 0x45, 0xca, 0xd0, 0xf3, 0xa5, 0x45, 0x38, 0xe9, 0x02, 0x85, 0x45,
            0xa9, 0x00, 0x18, 0x60, 0x20, 0xa2, 0xc7, 0xa2, 0x02, 0xa0, 0x00, 0xb1, 0x44, 0x8d,
            0xf3, 0xc0, 0xc8, 0xd0, 0xf8, 0xe6, 0x45, 0xca, 0xd0, 0xf3, 0xa5, 0x45, 0x38, 0xe9,
            0x02, 0x85, 0x45, 0xad, 0xf4, 0xc0, 0xd0, 0x02, 0x18, 0x60, 0x38, 0x60, 0xae, 0xf5,
            0xc0, 0xac, 0xf6, 0xc0, 0xa9, 0x00, 0x18, 0x60, 0xa5, 0x46, 0x8d, 0xf0, 0xc0, 0xa5,
            0x47, 0x8d, 0xf1, 0xc0, 0x60,
        ];
        let rom = hdd_rom(7);
        assert_eq!(rom[..golden.len()], golden);
        assert!(rom[golden.len()..0xfe].iter().all(|&b| b == 0));
        assert_eq!(rom[0xfe], 0x4f);
        assert_eq!(rom[0xff], 0x40);
        assert_eq!(hdd_driver_entry(7), 0xc740);
        assert_eq!(HDD_DRIVER_ENTRY, 0xc740);
    }

    /// A moved card patches exactly the boot signature-safe operands: the
    /// unit byte, the $Cn page of the internal JSRs, and the port low bytes.
    #[test]
    fn moved_rom_patches_the_slot_operands() {
        let rom = hdd_rom(5);
        assert_eq!(rom[0x01], 0x20, "boot signature survives the move");
        assert_eq!(rom[0x0d], 0x50); // unit = slot 5
        assert_eq!(rom[0x22], 0x50); // X = slot*16 at boot
        assert_eq!(rom[0x1e], 0xc5); // JSR $C540
        assert_eq!(rom[0x52], 0xc5); // JSR $C5A2
        assert_eq!(rom[0x54], 0xd2); // LDA $C0D2 (execute read)
        assert_eq!(rom[0x5d], 0xd3); // LDA $C0D3 (data)
        assert_eq!(rom[0xa5], 0xd0); // STA $C0D0 (block low)
        assert_eq!(hdd_driver_entry(5), 0xc540);
        // The slot-dependent operands are the only differences.
        let base = hdd_rom(7);
        let patched = [
            0x0d, 0x1e, 0x22, 0x52, 0x54, 0x5d, 0x76, 0x7e, 0x90, 0x99, 0x9c, 0xa5, 0xaa,
        ];
        for (i, (&a, &b)) in base.iter().zip(rom.iter()).enumerate() {
            if patched.contains(&i) {
                assert_ne!(a, b, "offset {i:#04x} should be slot-dependent");
            } else {
                assert_eq!(a, b, "unexpected difference at offset {i:#04x}");
            }
        }
    }
}
