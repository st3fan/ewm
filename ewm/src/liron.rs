//! The Apple II UniDisk 3.5 Controller ("Liron"), mounting .2mg images of
//! 800K or 400K. Like the hard-drive card in `crate::hdd` this is virtual
//! hardware: a 256-byte firmware ROM at `$Cn00` speaks the ProDOS
//! block-driver protocol ($42-$47) *and* the SmartPort call convention, and
//! pumps data byte-by-byte through magic I/O ports in the slot's DEVSEL
//! range — no IWM or drive-microcontroller emulation.
//!
//! The ROM carries the SmartPort identity: the boot signature `$Cn01=$20,
//! $Cn03=$00, $Cn05=$03` with the SmartPort marker `$Cn07=$00`, the ID type
//! byte at `$CnFB`, and `$CnFF` pointing at the ProDOS entry, with the
//! SmartPort dispatch at ProDOS entry + 3 — the convention ProDOS 8's
//! device scan relies on. Two drives, selected by the ProDOS unit byte's
//! high bit ($43 = DSSS0000) or the SmartPort unit number (1/2).
//!
//! SmartPort commands implemented: STATUS (device count, per-unit status +
//! block count, and the DIB), READ_BLOCK and WRITE_BLOCK; everything else
//! returns $21 (BADCMD). The dispatch borrows zero page $42-$45, which the
//! ProDOS block-driver convention already reserves for driver calls.
//!
//! Ports (DEVSEL low nibble; shown for slot 5 = $C0Dx):
//!   $C0D0 W  block bits 0-7        (writing any block byte resets the
//!   $C0D1 W  block bits 8-15        data index)
//!   $C0D2 R  execute READ: block -> buffer; returns a ProDOS error code
//!   $C0D3 RW data port, auto-incrementing through the 512-byte buffer
//!   $C0D4 R  execute WRITE: buffer -> image + file; returns error code
//!   $C0D5 R  selected drive's total blocks, low byte
//!   $C0D6 R  selected drive's total blocks, high byte
//!   $C0D7 W  ProDOS unit byte ($43): bit 7 selects drive 2
//!   $C0D8 W  SmartPort unit number (0 = the controller, 1/2 = drives)
//!   $C0D9 W  SmartPort STATUS code
//!   $C0DA R  execute SmartPort STATUS: payload -> buffer; returns error
//!   $C0DB R  SmartPort STATUS payload length

use std::collections::BTreeMap;
use std::io::{Seek, SeekFrom, Write};

use ewm_core::mem::Device;

/// A tiny two-pass assembler for the firmware: emit bytes, drop labels,
/// reference them from branches and absolute operands, resolve at the end.
/// The `hdd_rom` firmware is short enough for hand-counted offsets; this one
/// has enough forward branches that patching them mechanically is safer.
struct Asm {
    bytes: Vec<u8>,
    labels: BTreeMap<&'static str, usize>,
    /// (operand position, label, relative?)
    fixups: Vec<(usize, &'static str, bool)>,
    /// The $Cn page, for absolute operand high bytes.
    page: u8,
}

impl Asm {
    fn new(page: u8) -> Asm {
        Asm {
            bytes: Vec::new(),
            labels: BTreeMap::new(),
            fixups: Vec::new(),
            page,
        }
    }

    fn emit(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn label(&mut self, name: &'static str) {
        self.labels.insert(name, self.bytes.len());
    }

    /// A branch instruction with a label operand (opcode, then rel8).
    fn branch(&mut self, opcode: u8, target: &'static str) {
        self.emit(&[opcode]);
        self.fixups.push((self.bytes.len(), target, true));
        self.emit(&[0]);
    }

    /// JMP/JSR to a label within this $Cn page.
    fn abs(&mut self, opcode: u8, target: &'static str) {
        self.emit(&[opcode]);
        self.fixups.push((self.bytes.len(), target, false));
        self.emit(&[0, self.page]);
    }

    fn pad_to(&mut self, offset: usize) {
        assert!(self.bytes.len() <= offset, "firmware overran {offset:#x}");
        self.bytes.resize(offset, 0);
    }

    fn resolve(mut self) -> Vec<u8> {
        for (pos, label, relative) in &self.fixups {
            let target = self.labels[label];
            if *relative {
                let rel = target as isize - (*pos as isize + 1);
                self.bytes[*pos] = i8::try_from(rel).expect("branch out of range") as u8;
            } else {
                self.bytes[*pos] = target as u8;
            }
        }
        self.bytes
    }
}

// The firmware for a card in slot `n`, assembled by `Asm` from the listing
// below (labels in CAPS; `p(k)` is the DEVSEL port low byte $80+slot*16+k,
// `unit` is slot*16). Space is the whole game in this 256-byte page, so a
// few deliberate economies, all safe for the media this card takes:
//
// - SmartPort READ_BLOCK/WRITE_BLOCK translate onto the ProDOS driver: the
//   dispatch copies block/buffer from the parameter list into $44-$47 and
//   the command into $42, then joins the ProDOS path past its unit load.
// - The third SmartPort block byte is ignored (800K is 1600 blocks; the
//   device rejects anything past the media size).
// - The ProDOS driver does not preserve $45 across the transfer and does
//   not report offline drives from STATUS (they show 0 blocks; reads of an
//   empty drive fail with $2F where it matters). ProDOS rebuilds $42-$47
//   for every call.
//
// The ProDOS driver body sits behind a JMP at the entry point so the
// SmartPort dispatch lands at entry+3, where ProDOS 8 expects it; the boot
// gap below $0040 houses the SmartPort block-call setup.
//
//   ; signature: $20/$00/$03 at 1/3/5 and the SmartPort marker $00 at 7
//   0000:  A9 20     LDA #$20
//          A9 00     LDA #$00
//          A9 03     LDA #$03
//          A9 00     LDA #$00
//   ; boot: READ block 0 of drive 1 to $0800, then JMP $0801. The $42-$47
//   ; parameter block comes from BOOTTAB.
//          A2 05     LDX #$05
//   BOOTLP: BD tb Cn LDA BOOTTAB,X
//          95 42     STA $42,X
//          CA        DEX
//          10 F8     BPL BOOTLP
//          20 40 Cn  JSR ENTRY
//          B0 05     BCS fail
//          A2 un     LDX #unit
//          4C 01 08  JMP $0801
//   fail:  4C 00 E0  JMP $E000
//   BOOTTAB: 01 un 00 08 00 00     ; READ, unit, buffer $0800, block 0
//   SPBLOCK:                       ; SmartPort block call -> ProDOS driver
//          A0 05     LDY #$05
//          B1 42     LDA ($42),Y   ; block mid
//          85 47     STA $47
//          88 B1 42  85 46         ; block lo
//          88 B1 42  85 45         ; buffer hi
//          88 B1 42  85 44         ; buffer lo
//          86 42     STX $42       ; the command (1/2, shared numbering)
//          4C PD2    JMP PD2       ; drive already selected via the port
//
//   0040: ENTRY:  4C PD    JMP PD  ; the ProDOS driver
//   0043: SP:                      ; SmartPort dispatch = ENTRY+3
//          68 85 42  PLA/STA $42   ; $42/$43 = return-1 (the JSR's inline
//          68 85 43  PLA/STA $43   ;  cmd/list bytes)
//          A0 01     LDY #$01
//          B1 42     LDA ($42),Y   ; command byte
//          AA        TAX
//          18        CLC           ; push return+3, past the inline bytes
//          A5 42 69 03 A8          ; LDA/ADC #3/TAY
//          A5 43 69 00 48 98 48    ; LDA/ADC #0/PHA, TYA/PHA
//          A0 02     LDY #$02      ; $42/$43 = the parameter list
//          B1 42 48  LDA ($42),Y / PHA
//          C8 B1 42 85 43          ; list hi
//          68 85 42                ; list lo
//          A0 01     LDY #$01
//          B1 42     LDA ($42),Y   ; unit number
//          8D p8     STA unit port ; selects the drive (0 = controller)
//          8A        TXA
//          F0 SPST   BEQ SPSTATUS  ; 0 = STATUS
//          C9 03     CMP #$03
//          90 SPBL   BCC SPBLOCK   ; 1/2 = READ_BLOCK/WRITE_BLOCK
//          A9 21     LDA #$21      ; anything else: BADCMD
//          38 60     SEC/RTS
//   SPSTATUS:
//          A0 04     LDY #$04
//          B1 42     LDA ($42),Y   ; status code
//          8D p9     STA statcode port
//          A0 02 B1 42 85 44       ; status list lo
//          C8 B1 42 85 45          ; status list hi
//          AD pA     LDA execute-status
//          D0 PERR   BNE PERR
//          AE pB     LDX payload length
//          A0 00     LDY #$00
//   SPLP:  AD p3 91 44 C8 CA       ; data port -> (list),Y
//          D0 SPLP   BNE SPLP
//          A9 00 18 60             ; success
//   PD:    A5 43     LDA $43
//          8D p7     STA unit port ; bit 7 selects the drive
//   PD2:   A5 46 8D p0             ; block ports from $46/$47
//          A5 47 8D p1
//          A5 42     LDA $42       ; command
//          F0 PST    BEQ PSTATUS   ; 0 = STATUS
//          4A        LSR           ; 1 -> C=1,A=0; 2 -> C=0; 3 -> C=1,A=1
//          90 PWR    BCC PWRITE    ; 2 = WRITE
//          F0 PRD    BEQ PREAD     ; 1 = READ
//          A9 00 18 60             ; 3 = FORMAT: succeed as a no-op
//   PREAD: AD p2     LDA execute-read
//          D0 PERR   BNE PERR
//          A2 02 A0 00             ; pump 2 pages to ($44)
//   RLP:   AD p3 91 44 C8 D0 F8 / E6 45 CA D0 F3
//          A9 00 18 60
//   PWRITE: A2 02 A0 00            ; pump 2 pages from ($44), then commit
//   WLP:   B1 44 8D p3 C8 D0 F8 / E6 45 CA D0 F3
//          AD p4     LDA execute-write
//          D0 PERR   BNE PERR
//          18 60
//   PSTATUS:
//          AE p5 AC p6             ; blocks in X/Y (0 for an empty drive)
//          18 60                   ; A is already 0, the routing command
//   PERR:  38 60                   ; error code in A
//
//   ; ID bytes
//   00FB: 00        SmartPort ID type (plain SmartPort)
//   00FC: 00 00     block count: "ask STATUS"
//   00FE: DF        attributes: removable, interruptible, 2 volumes,
//                   supports format/write/read/status
//   00FF: 40        ProDOS entry offset
pub fn liron_rom(slot: u8) -> [u8; 256] {
    let unit = slot << 4;
    let n = 0xc0 + slot;
    let p = |k: u8| 0x80 + slot * 16 + k;
    let mut a = Asm::new(n);

    // Boot half.
    a.emit(&[0xa9, 0x20, 0xa9, 0x00, 0xa9, 0x03, 0xa9, 0x00]);
    a.emit(&[0xa2, 0x05]);
    a.label("BOOTLP");
    a.abs(0xbd, "BOOTTAB"); // LDA BOOTTAB,X
    a.emit(&[0x95, 0x42, 0xca]);
    a.branch(0x10, "BOOTLP");
    a.abs(0x20, "ENTRY");
    a.branch(0xb0, "BOOTFAIL");
    a.emit(&[0xa2, unit, 0x4c, 0x01, 0x08]);
    a.label("BOOTFAIL");
    a.emit(&[0x4c, 0x00, 0xe0]);
    a.label("BOOTTAB");
    a.emit(&[0x01, unit, 0x00, 0x08, 0x00, 0x00]); // $42-$47: READ block 0

    // The boot gap houses the SmartPort block-call setup; the dispatch's
    // BCC reaches back here, and its exit is a JMP.
    a.label("SPBLOCK");
    a.emit(&[0xa0, 0x05, 0xb1, 0x42, 0x85, 0x47]); // block mid
    a.emit(&[0x88, 0xb1, 0x42, 0x85, 0x46]); // block lo
    a.emit(&[0x88, 0xb1, 0x42, 0x85, 0x45]); // buffer hi
    a.emit(&[0x88, 0xb1, 0x42, 0x85, 0x44]); // buffer lo
    a.emit(&[0x86, 0x42]); // the command, 1/2 in both protocols
    a.abs(0x4c, "PD2");

    // The ProDOS entry at $40, with the SmartPort dispatch at entry+3.
    a.pad_to(0x40);
    a.label("ENTRY");
    a.abs(0x4c, "PD");
    a.label("SP");
    a.emit(&[0x68, 0x85, 0x42, 0x68, 0x85, 0x43]); // ptr = return-1
    a.emit(&[0xa0, 0x01, 0xb1, 0x42, 0xaa]); // X = command
    a.emit(&[0x18, 0xa5, 0x42, 0x69, 0x03, 0xa8]); // return+3, low in Y
    a.emit(&[0xa5, 0x43, 0x69, 0x00, 0x48, 0x98, 0x48]); // push hi, lo
    a.emit(&[0xa0, 0x02, 0xb1, 0x42, 0x48]); // list lo, stashed
    a.emit(&[0xc8, 0xb1, 0x42, 0x85, 0x43]); // list hi
    a.emit(&[0x68, 0x85, 0x42]); // $42/$43 = list
    a.emit(&[0xa0, 0x01, 0xb1, 0x42, 0x8d, p(8), 0xc0]); // unit port
    a.emit(&[0x8a]);
    a.branch(0xf0, "SPSTATUS");
    a.emit(&[0xc9, 0x03]);
    a.branch(0x90, "SPBLOCK");
    a.emit(&[0xa9, 0x21, 0x38, 0x60]); // BADCMD

    a.label("SPSTATUS");
    a.emit(&[0xa0, 0x04, 0xb1, 0x42, 0x8d, p(9), 0xc0]); // status code
    a.emit(&[0xa0, 0x02, 0xb1, 0x42, 0x85, 0x44]); // status list lo
    a.emit(&[0xc8, 0xb1, 0x42, 0x85, 0x45]); // status list hi
    a.emit(&[0xad, p(0xa), 0xc0]); // execute
    a.branch(0xd0, "PERR");
    a.emit(&[0xae, p(0xb), 0xc0, 0xa0, 0x00]); // X = length, Y = 0
    a.label("SPLP");
    a.emit(&[0xad, p(3), 0xc0, 0x91, 0x44, 0xc8, 0xca]);
    a.branch(0xd0, "SPLP");
    a.emit(&[0xa9, 0x00, 0x18, 0x60]);

    // The ProDOS block driver. PD2 skips the unit load for SmartPort block
    // calls, whose unit went through the port in the dispatch.
    a.label("PD");
    a.emit(&[0xa5, 0x43, 0x8d, p(7), 0xc0]); // unit byte -> drive select
    a.label("PD2");
    a.emit(&[0xa5, 0x46, 0x8d, p(0), 0xc0]); // block ports from $46/$47
    a.emit(&[0xa5, 0x47, 0x8d, p(1), 0xc0]);
    a.emit(&[0xa5, 0x42]);
    a.branch(0xf0, "PSTATUS");
    a.emit(&[0x4a]); // LSR: 1 -> C=1,A=0; 2 -> C=0; 3 -> C=1,A=1
    a.branch(0x90, "PWRITE");
    a.branch(0xf0, "PREAD");
    a.emit(&[0xa9, 0x00, 0x18, 0x60]); // FORMAT: succeed as a no-op

    a.label("PREAD");
    a.emit(&[0xad, p(2), 0xc0]); // execute read
    a.branch(0xd0, "PERR");
    a.emit(&[0xa2, 0x02, 0xa0, 0x00]); // two pages
    a.emit(&[0xad, p(3), 0xc0, 0x91, 0x44, 0xc8, 0xd0, 0xf8]);
    a.emit(&[0xe6, 0x45, 0xca, 0xd0, 0xf3]);
    a.emit(&[0xa9, 0x00, 0x18, 0x60]);

    a.label("PWRITE");
    a.emit(&[0xa2, 0x02, 0xa0, 0x00]); // two pages
    a.emit(&[0xb1, 0x44, 0x8d, p(3), 0xc0, 0xc8, 0xd0, 0xf8]);
    a.emit(&[0xe6, 0x45, 0xca, 0xd0, 0xf3]);
    a.emit(&[0xad, p(4), 0xc0]); // commit
    a.branch(0xd0, "PERR");
    a.emit(&[0x18, 0x60]);

    a.label("PSTATUS");
    a.emit(&[0xae, p(5), 0xc0, 0xac, p(6), 0xc0]); // blocks in X/Y
    a.emit(&[0x18, 0x60]); // A is already 0 (the command that routed here)
    a.label("PERR");
    a.emit(&[0x38, 0x60]);

    let code = a.resolve();
    assert!(code.len() <= 0xfb, "firmware must leave room for ID bytes");
    let mut rom = [0u8; 256];
    rom[..code.len()].copy_from_slice(&code);
    rom[0xfb] = 0x00; // SmartPort ID type: plain SmartPort
    rom[0xfe] = 0xdf; // removable, interruptible, 2 volumes, all calls
    rom[0xff] = 0x40; // ProDOS entry offset
    rom
}

/// The ProDOS block-driver entry within the slot ROM (what `$CnFF`
/// publishes).
pub const fn liron_prodos_entry(slot: u8) -> u16 {
    ((0xc0 + slot as u16) << 8) | 0x40
}

/// The SmartPort dispatch entry: ProDOS entry + 3, per the convention.
pub const fn liron_smartport_entry(slot: u8) -> u16 {
    liron_prodos_entry(slot) + 3
}

// ProDOS MLI / SmartPort error codes returned by the device.
const ERR_IO: u8 = 0x27;
const ERR_NO_DEVICE: u8 = 0x28;
const ERR_WRITE_PROTECTED: u8 = 0x2b;
const ERR_OFFLINE: u8 = 0x2f;

/// One mounted .2mg image: the decoded blocks plus what write-back needs.
struct Image {
    data: Vec<u8>,
    blocks: u16,
    /// Start of the block data within the file (past the 2IMG header).
    data_offset: u64,
    /// The image file, kept open for write-back; `None` when the 2mg is
    /// locked or the file is not writable.
    file: Option<std::fs::File>,
}

impl Image {
    /// Parse a .2mg file: 64-byte `2IMG` header, ProDOS-order data, and a
    /// block count of 800 (400K) or 1600 (800K) — the UniDisk 3.5 media
    /// sizes.
    fn open(path: &str) -> Result<Image, String> {
        let raw = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        if raw.len() < 64 || &raw[0..4] != b"2IMG" {
            return Err(format!("{path}: not a 2IMG image (bad magic)"));
        }
        let le32 = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
        let format = le32(0x0c);
        if format != 1 {
            return Err(format!(
                "{path}: 2IMG format {format} is not ProDOS-order (only format 1 mounts)"
            ));
        }
        let flags = le32(0x10);
        let blocks = le32(0x14);
        let data_offset = le32(0x18) as usize;
        let data_len = le32(0x1c) as usize;
        if blocks != 800 && blocks != 1600 {
            return Err(format!(
                "{path}: {blocks} blocks; a UniDisk 3.5 takes 400K (800) or 800K (1600) images"
            ));
        }
        if data_len != blocks as usize * 512 || raw.len() < data_offset + data_len {
            return Err(format!(
                "{path}: truncated 2IMG (data length/offset mismatch)"
            ));
        }
        let locked = flags & 0x8000_0000 != 0;
        let file = if locked {
            None
        } else {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path);
            if file.is_err() {
                eprintln!("[LIRON] {path} is not writable; mounting read-only");
            }
            file.ok()
        };
        Ok(Image {
            data: raw[data_offset..data_offset + data_len].to_vec(),
            blocks: blocks as u16,
            data_offset: data_offset as u64,
            file,
        })
    }
}

pub struct Liron {
    drives: [Option<Image>; 2],
    /// The drive block operations address, from the ProDOS unit byte's bit
    /// 7 or the SmartPort unit number.
    drive: usize,
    /// Whether the last unit selection was valid for block operations
    /// (SmartPort unit 0 — the controller itself — is not).
    unit_ok: bool,
    /// The last SmartPort unit selection (0 = the controller).
    sp_unit: u8,
    statcode: u8,
    block: u16,
    buf: [u8; 512],
    index: usize,
    /// SmartPort STATUS payload length after an execute.
    status_len: u8,
}

impl Liron {
    /// A controller with both drives empty, like a Disk II: media mounts
    /// afterwards with `load`.
    pub fn new() -> Liron {
        Liron {
            drives: [None, None],
            drive: 0,
            unit_ok: true,
            sp_unit: 1,
            statcode: 0,
            block: 0,
            buf: [0; 512],
            index: 0,
            status_len: 0,
        }
    }

    /// Mount a .2mg image in drive 0 or 1, replacing what was there.
    pub fn load(&mut self, drive: usize, path: &str) -> Result<(), String> {
        self.drives[drive] = Some(Image::open(path)?);
        Ok(())
    }

    /// The mounted image's block count — 800 or 1600 — or `None` for an
    /// empty drive (for the WozBug SLOTS display).
    pub fn drive_blocks(&self, drive: usize) -> Option<u16> {
        self.drives[drive].as_ref().map(|i| i.blocks)
    }

    fn selected(&self) -> Option<&Image> {
        if !self.unit_ok {
            return None;
        }
        self.drives[self.drive].as_ref()
    }

    fn execute_read(&mut self) -> u8 {
        self.index = 0;
        if !self.unit_ok {
            return ERR_NO_DEVICE;
        }
        let Some(image) = self.drives[self.drive].as_ref() else {
            return ERR_OFFLINE;
        };
        if self.block >= image.blocks {
            return ERR_IO;
        }
        let off = self.block as usize * 512;
        self.buf.copy_from_slice(&image.data[off..off + 512]);
        0
    }

    /// Commit the buffered block to the image and write it back into the
    /// .2mg file at `data_offset + block * 512`, leaving the header alone.
    fn execute_write(&mut self) -> u8 {
        self.index = 0;
        if !self.unit_ok {
            return ERR_NO_DEVICE;
        }
        let Some(image) = self.drives[self.drive].as_mut() else {
            return ERR_OFFLINE;
        };
        if self.block >= image.blocks {
            return ERR_IO;
        }
        let Some(file) = &mut image.file else {
            return ERR_WRITE_PROTECTED;
        };
        let off = self.block as usize * 512;
        image.data[off..off + 512].copy_from_slice(&self.buf);
        let written = file
            .seek(SeekFrom::Start(image.data_offset + off as u64))
            .and_then(|_| file.write_all(&self.buf))
            .and_then(|_| file.flush());
        match written {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("[LIRON] write of block {} failed: {e}", self.block);
                ERR_IO
            }
        }
    }

    /// Execute a SmartPort STATUS: fill the buffer with the payload for the
    /// selected unit and status code, per the SmartPort conventions ProDOS 8
    /// uses when scanning the slot.
    fn execute_status(&mut self) -> u8 {
        self.index = 0;
        self.status_len = 0;
        match (self.sp_unit, self.statcode) {
            // Unit 0, code 0: the controller — device count plus reserved
            // bytes. Two UniDisk 3.5s are always attached; media is what
            // comes and goes.
            (0, 0) => {
                let payload = [2u8, 0, 0, 0, 0, 0, 0, 0];
                self.set_status_payload(&payload);
                0
            }
            (1 | 2, 0) => {
                let payload = self.device_status(self.sp_unit as usize - 1);
                self.set_status_payload(&payload);
                0
            }
            // Code 3: the Device Information Block.
            (1 | 2, 3) => {
                let mut payload = [0u8; 25];
                payload[..4].copy_from_slice(&self.device_status(self.sp_unit as usize - 1));
                let name = b"UNIDISK 3.5     ";
                payload[4] = 11; // name length
                payload[5..21].copy_from_slice(name);
                payload[21] = 0x01; // device type: 3.5 disk
                payload[22] = 0xc0; // subtype: removable, extended-capable
                payload[23] = 0x01; // firmware version
                payload[24] = 0x00;
                self.set_status_payload(&payload);
                0
            }
            _ => 0x21, // BADCTL / bad command for anything else
        }
    }

    /// The 4-byte device status: the SmartPort status byte plus a 3-byte
    /// block count.
    fn device_status(&self, drive: usize) -> [u8; 4] {
        match &self.drives[drive] {
            Some(image) => {
                // Block device, read/write allowed, online, format allowed —
                // with the write bits swapped for protected media.
                let writable = image.file.is_some();
                let mut status = 0b1011_1000; // block + read + online + format
                if writable {
                    status |= 0b0100_0000; // write allowed
                } else {
                    status |= 0b0000_0100; // write protected
                }
                let b = image.blocks as u32;
                [status, b as u8, (b >> 8) as u8, (b >> 16) as u8]
            }
            // Block device, present but no media.
            None => [0b1010_1000, 0, 0, 0],
        }
    }

    fn set_status_payload(&mut self, payload: &[u8]) {
        self.buf[..payload.len()].copy_from_slice(payload);
        self.status_len = payload.len() as u8;
    }
}

impl Default for Liron {
    fn default() -> Liron {
        Liron::new()
    }
}

/// The card as an I/O device over its slot's 16-byte DEVSEL range; only the
/// low nibble is decoded, so the same device works in any slot.
impl Device for Liron {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        match addr & 0x0f {
            0x2 => self.execute_read(),
            0x3 => {
                let b = self.buf[self.index % 512];
                self.index = (self.index + 1) % 512;
                b
            }
            0x4 => self.execute_write(),
            0x5 => self
                .selected()
                .map(|i| (i.blocks & 0xff) as u8)
                .unwrap_or(0),
            0x6 => self.selected().map(|i| (i.blocks >> 8) as u8).unwrap_or(0),
            0xa => self.execute_status(),
            0xb => self.status_len,
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
            0x7 => {
                // The ProDOS unit byte: bit 7 selects drive 2.
                self.drive = (b >> 7) as usize;
                self.unit_ok = true;
                self.sp_unit = self.drive as u8 + 1;
            }
            0x8 => {
                // The SmartPort unit number: 0 is the controller (valid for
                // STATUS only), 1/2 the drives.
                self.sp_unit = b;
                match b {
                    1 | 2 => {
                        self.drive = b as usize - 1;
                        self.unit_ok = true;
                    }
                    _ => {
                        self.drive = 0;
                        self.unit_ok = false;
                    }
                }
            }
            0x9 => self.statcode = b,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A .2mg of `blocks` 512-byte blocks, block *b* filled with byte *b*,
    /// optionally locked.
    fn make_2mg(name: &str, blocks: u32, locked: bool) -> String {
        let mut raw = vec![0u8; 64];
        raw[0..4].copy_from_slice(b"2IMG");
        raw[4..8].copy_from_slice(b"EWM!");
        raw[8..10].copy_from_slice(&64u16.to_le_bytes());
        raw[10..12].copy_from_slice(&1u16.to_le_bytes());
        raw[0x0c..0x10].copy_from_slice(&1u32.to_le_bytes()); // ProDOS order
        let flags: u32 = if locked { 0x8000_0000 } else { 0 };
        raw[0x10..0x14].copy_from_slice(&flags.to_le_bytes());
        raw[0x14..0x18].copy_from_slice(&blocks.to_le_bytes());
        raw[0x18..0x1c].copy_from_slice(&64u32.to_le_bytes());
        raw[0x1c..0x20].copy_from_slice(&(blocks * 512).to_le_bytes());
        for b in 0..blocks {
            raw.extend(std::iter::repeat_n(b as u8, 512));
        }
        let path =
            std::env::temp_dir().join(format!("ewm-liron-test-{name}-{}.2mg", std::process::id()));
        std::fs::write(&path, &raw).expect("cannot write test image");
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn accepts_400k_and_800k_and_rejects_the_rest() {
        let path = make_2mg("800k", 1600, false);
        let image = Image::open(&path).expect("800K must mount");
        assert_eq!(image.blocks, 1600);
        assert_eq!(image.data_offset, 64);
        std::fs::remove_file(&path).ok();

        let path = make_2mg("400k", 800, false);
        assert_eq!(Image::open(&path).expect("400K must mount").blocks, 800);
        std::fs::remove_file(&path).ok();

        let path = make_2mg("140k", 280, false);
        let err = Image::open(&path).err().expect("must be rejected");
        assert!(err.contains("400K (800) or 800K (1600)"), "{err}");
        std::fs::remove_file(&path).ok();

        let path = make_2mg("dos", 800, false);
        let mut raw = std::fs::read(&path).unwrap();
        raw[0x0c] = 0; // DOS order
        std::fs::write(&path, &raw).unwrap();
        let err = Image::open(&path).err().expect("must be rejected");
        assert!(err.contains("not ProDOS-order"), "{err}");
        raw[0..4].copy_from_slice(b"NOPE");
        std::fs::write(&path, &raw).unwrap();
        let err = Image::open(&path).err().expect("must be rejected");
        assert!(err.contains("bad magic"), "{err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn ports_read_blocks_per_drive_and_write_back() {
        let p1 = make_2mg("d1", 800, false);
        let p2 = make_2mg("d2", 1600, false);
        let mut liron = Liron::new();
        liron.load(0, &p1).unwrap();
        liron.load(1, &p2).unwrap();

        // Drive 1 via the ProDOS unit byte, block 5.
        liron.write(0xc0d7, 0x50, 0);
        liron.write(0xc0d0, 5, 0);
        liron.write(0xc0d1, 0, 0);
        assert_eq!(liron.read(0xc0d2, 0), 0);
        assert_eq!(liron.read(0xc0d3, 0), 5);
        assert_eq!(liron.read(0xc0d5, 0), (800u16 & 0xff) as u8);
        assert_eq!(liron.read(0xc0d6, 0), (800u16 >> 8) as u8);

        // Drive 2 via the drive bit; its blocks differ.
        liron.write(0xc0d7, 0xd0, 0);
        assert_eq!(liron.read(0xc0d2, 0), 0);
        assert_eq!(liron.read(0xc0d3, 0), 5);
        assert_eq!(liron.read(0xc0d6, 0), (1600u16 >> 8) as u8);

        // Write block 7 of drive 2 and check the file at data_offset.
        liron.write(0xc0d0, 7, 0);
        for _ in 0..512 {
            liron.write(0xc0d3, 0xa5, 0);
        }
        assert_eq!(liron.read(0xc0d4, 0), 0, "write must commit");
        let raw = std::fs::read(&p2).unwrap();
        assert_eq!(raw[64 + 7 * 512], 0xa5, "write lands past the header");
        assert_eq!(raw[64 + 6 * 512], 6, "the neighboring block is intact");

        // An out-of-range block errors.
        liron.write(0xc0d0, (1600u16 & 0xff) as u8, 0);
        liron.write(0xc0d1, (1600u16 >> 8) as u8, 0);
        assert_eq!(liron.read(0xc0d2, 0), ERR_IO);

        std::fs::remove_file(&p1).ok();
        std::fs::remove_file(&p2).ok();
    }

    #[test]
    fn empty_and_locked_drives_error_properly() {
        let mut liron = Liron::new();
        liron.write(0xc0d7, 0x50, 0);
        assert_eq!(liron.read(0xc0d2, 0), ERR_OFFLINE, "no media");

        let path = make_2mg("locked", 800, true);
        liron.load(0, &path).unwrap();
        assert_eq!(liron.read(0xc0d2, 0), 0, "locked media reads");
        assert_eq!(
            liron.read(0xc0d4, 0),
            ERR_WRITE_PROTECTED,
            "locked media must not write"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn smartport_status_payloads() {
        let path = make_2mg("status", 1600, false);
        let mut liron = Liron::new();
        liron.load(0, &path).unwrap();

        // Unit 0: the controller, two devices.
        liron.write(0xc0d8, 0, 0);
        liron.write(0xc0d9, 0, 0);
        assert_eq!(liron.read(0xc0da, 0), 0);
        assert_eq!(liron.read(0xc0db, 0), 8);
        assert_eq!(liron.read(0xc0d3, 0), 2, "device count");

        // Unit 1: online, writable, 1600 blocks little-endian.
        liron.write(0xc0d8, 1, 0);
        assert_eq!(liron.read(0xc0da, 0), 0);
        assert_eq!(liron.read(0xc0db, 0), 4);
        let status = liron.read(0xc0d3, 0);
        assert_eq!(status & 0b1101_1000, 0b1101_1000, "block+write+read+online");
        assert_eq!(liron.read(0xc0d3, 0), (1600u32 & 0xff) as u8);
        assert_eq!(liron.read(0xc0d3, 0), (1600u32 >> 8) as u8);
        assert_eq!(liron.read(0xc0d3, 0), 0);

        // Unit 2: no media — online bit clear.
        liron.write(0xc0d8, 2, 0);
        assert_eq!(liron.read(0xc0da, 0), 0);
        let status = liron.read(0xc0d3, 0);
        assert_eq!(status & 0b0001_0000, 0, "offline");

        // The DIB names the drive.
        liron.write(0xc0d8, 1, 0);
        liron.write(0xc0d9, 3, 0);
        assert_eq!(liron.read(0xc0da, 0), 0);
        assert_eq!(liron.read(0xc0db, 0), 25);
        let mut dib = [0u8; 25];
        for b in dib.iter_mut() {
            *b = liron.read(0xc0d3, 0);
        }
        assert_eq!(dib[4], 11);
        assert_eq!(&dib[5..16], b"UNIDISK 3.5");
        assert_eq!(dib[21], 0x01, "device type: 3.5 disk");

        // SmartPort block operations refuse unit 0.
        liron.write(0xc0d8, 0, 0);
        assert_eq!(liron.read(0xc0d2, 0), ERR_NO_DEVICE);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn rom_carries_the_smartport_identity() {
        let rom = liron_rom(5);
        // Boot signature with the SmartPort marker at $Cn07.
        assert_eq!(rom[0x01], 0x20);
        assert_eq!(rom[0x03], 0x00);
        assert_eq!(rom[0x05], 0x03);
        assert_eq!(rom[0x07], 0x00);
        // The ProDOS entry is a JMP; the SmartPort dispatch (entry+3)
        // starts with PLA.
        assert_eq!(rom[0x40], 0x4c);
        assert_eq!(rom[0x43], 0x68);
        assert_eq!(liron_prodos_entry(5), 0xc540);
        assert_eq!(liron_smartport_entry(5), 0xc543);
        // ID bytes.
        assert_eq!(rom[0xfb], 0x00);
        assert_eq!(rom[0xfc], 0x00);
        assert_eq!(rom[0xfd], 0x00);
        assert_eq!(rom[0xfe], 0xdf);
        assert_eq!(rom[0xff], 0x40);
    }

    /// The slot-5 and slot-7 ROMs differ only in slot-dependent operands:
    /// the unit immediates, $Cn page high bytes, and DEVSEL port low bytes.
    #[test]
    fn moved_rom_patches_only_slot_operands() {
        let a = liron_rom(5);
        let b = liron_rom(7);
        for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
            if x == y {
                continue;
            }
            let slot_dependent = (x == 0x50 && y == 0x70)   // unit bytes
                || (x == 0xc5 && y == 0xc7)                 // $Cn pages
                || (x & 0xf0 == 0xd0 && y & 0xf0 == 0xf0); // port low bytes
            assert!(
                slot_dependent,
                "unexpected difference at {i:#04x}: {x:#04x} vs {y:#04x}"
            );
        }
    }
}
