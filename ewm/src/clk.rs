//! Thunderclock Plus real-time clock (slot 1 by default, usable in any
//! slot). Like the hard disk this is virtual hardware, not a uPD1990AC chip
//! simulation: a 256-byte firmware ROM at $Cn00 speaks the small protocol
//! the ProDOS built-in clock driver expects, and pulls the host's local
//! time byte-by-byte through two I/O ports in the slot's DEVSEL range
//! ($C080 + slot*16).
//!
//! ProDOS recognizes a clock card by the ID bytes $Cn00=$08, $Cn02=$28,
//! $Cn04=$58, $Cn06=$70. When it finds one it installs a JMP at $BF06 to the
//! built-in Thunderclock driver, which — for a card in slot n — calls
//! `JSR $Cn0B` (with A = '#'|$80, the numeric-format command) then
//! `JSR $Cn08` (read). The read must deposit an ASCII string at $0200 of the
//! form `MO,DW,DT,HH,MM,SS` — two digits per field, day-of-week 00(Sun)..
//! 06(Sat) — with the *ones* digit of every field carrying the high bit
//! ($B0-$B9), which is how the driver's `... SBC #$B0` recovers the value.
//! The driver reads only offsets 0,1,3,4,6,7,9,10,12,13 (month, day-of-week,
//! date, hour, minute); the separators, seconds, and trailing CR are for
//! fidelity with the real card's `#` output and are ignored.
//!
//! The uPD1990AC has no year register, so ProDOS derives the year from the
//! month, date, and day-of-week via a seven-entry table. In ProDOS 2.4.x
//! that table covers 2023-2028, so as long as this card reports the correct
//! day-of-week the displayed year is right for any host date in that window;
//! outside it the year is wrong (a driver limitation) but the month, date,
//! and time stay correct.

use ewm_core::mem::Device;

// The firmware for a card in slot `n`. Hand-assembled; the listing below is
// the source of truth for the bytes (shown for slot 1 — the three
// slot-dependent operands are computed from `slot`). The ID bytes at the
// even offsets double as harmless opcodes, as they do on the real card;
// $Cn01=$78 (not $20) is what keeps the Autostart slot scan from mistaking
// this ROM for a bootable Disk II card.
//
//   ; ID bytes / opcodes
//   C100: 08        PHP            ; ID $Cn00 = $08
//   C101: 78        SEI            ; not $20: breaks the Disk II boot signature
//   C102: 28        PLP            ; ID $Cn02 = $28
//   C103: 18        CLC
//   C104: 58        CLI            ; ID $Cn04 = $58
//   C105: EA        NOP
//   C106: 70 08     BVS $C110      ; ID $Cn06 = $70
//   ; ProDOS entry points
//   C108: 4C 10 C1  JMP $Cn10      ; READ: deposit the time string at $0200
//   C10B: 60        RTS            ; WRITE: format commands acknowledged, ignored
//   C10C: EA EA EA EA              ; pad
//   ; READ: latch the host clock, copy the pre-formatted string to $0200
//   C110: 8D 90 C0  STA $C090+s    ; latch local time + reset index (value ignored)
//   C113: A0 00     LDY #$00
//   C115: AD 91 C0  LDA $C091+s    ; next string byte; $00 = end
//   C118: F0 06     BEQ $C120
//   C11A: 99 00 02  STA $0200,Y
//   C11D: C8        INY
//   C11E: D0 F5     BNE $C115
//   C120: 60        RTS
pub fn clk_rom(slot: u8) -> [u8; 256] {
    let n = 0xc0 + slot; // the $Cn page the ROM lives in
    let latch = 0x80 + slot * 16; // low byte of the DEVSEL latch port
    let data = latch + 1; // low byte of the DEVSEL data port
    let mut rom = [0u8; 256];
    let code: [u8; 33] = [
        0x08, 0x78, 0x28, 0x18, 0x58, 0xea, 0x70, 0x08, // ID bytes / opcodes
        0x4c, 0x10, n, // JMP $Cn10
        0x60, 0xea, 0xea, 0xea, 0xea, // RTS + pad
        0x8d, latch, 0xc0, // STA $C0xx (latch)
        0xa0, 0x00, // LDY #$00
        0xad, data, 0xc0, // LDA $C0xx (data)
        0xf0, 0x06, // BEQ +6
        0x99, 0x00, 0x02, // STA $0200,Y
        0xc8, // INY
        0xd0, 0xf5, // BNE -11
        0x60, // RTS
    ];
    rom[..code.len()].copy_from_slice(&code);
    rom
}

/// The ProDOS read entry point within the slot ROM ($Cn00 + $08).
pub const fn clk_read_entry(slot: u8) -> u16 {
    ((0xc0 + slot as u16) << 8) | 0x08
}

/// The ProDOS read entry point for the default slot-1 card.
pub const CLK_READ_ENTRY: u16 = clk_read_entry(1);

/// The 18-byte string the card serves: `MO,DW,DT,HH,MM,SS` + CR, every byte
/// high-bit set (the real card emits GETLN-style high ASCII).
const STRING_LEN: usize = 18;

/// One latched local-time reading, in the fields the Thunderclock reports.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClockTime {
    /// 1-12
    pub month: u8,
    /// 0 = Sunday .. 6 = Saturday
    pub weekday: u8,
    /// 1-31
    pub day: u8,
    /// 0-23
    pub hour: u8,
    /// 0-59
    pub minute: u8,
    /// 0-59
    pub second: u8,
}

impl ClockTime {
    /// The host's current local time.
    fn now() -> ClockTime {
        use chrono::{Datelike, Local, Timelike};
        let now = Local::now();
        ClockTime {
            month: now.month() as u8,
            weekday: now.weekday().num_days_from_sunday() as u8,
            day: now.day() as u8,
            hour: now.hour() as u8,
            minute: now.minute() as u8,
            second: now.second() as u8,
        }
    }

    /// Format as the card's numeric-mode string. Two high-ASCII digits per
    /// field, `,` separators, trailing CR — exactly the bytes the ProDOS
    /// driver parses out of $0200.
    fn to_string_bytes(self) -> [u8; STRING_LEN] {
        // High-bit ASCII digit pair for a 0-99 value.
        fn pair(value: u8, out: &mut [u8], at: usize) {
            out[at] = 0xb0 + (value / 10) % 10;
            out[at + 1] = 0xb0 + value % 10;
        }
        let mut s = [0u8; STRING_LEN];
        pair(self.month, &mut s, 0);
        s[2] = 0xac; // ','
        pair(self.weekday, &mut s, 3);
        s[5] = 0xac;
        pair(self.day, &mut s, 6);
        s[8] = 0xac;
        pair(self.hour, &mut s, 9);
        s[11] = 0xac;
        pair(self.minute, &mut s, 12);
        s[14] = 0xac;
        pair(self.second, &mut s, 15);
        s[17] = 0x8d; // CR
        s
    }
}

pub struct Clk {
    /// A fixed time for deterministic tests; `None` reads the host clock.
    fixed: Option<ClockTime>,
    buf: [u8; STRING_LEN],
    /// Next byte to serve; `== STRING_LEN` once the string is exhausted.
    index: usize,
}

impl Clk {
    pub fn new() -> Clk {
        Clk {
            fixed: None,
            buf: [0; STRING_LEN],
            index: STRING_LEN,
        }
    }

    /// Pin the clock to a fixed reading (tests); the next latch uses it.
    pub fn set_fixed_time(&mut self, time: ClockTime) {
        self.fixed = Some(time);
    }

    /// Sample the clock and format it into `buf`, resetting the read index —
    /// the `$C090` latch.
    fn latch(&mut self) {
        let time = self.fixed.unwrap_or_else(ClockTime::now);
        self.buf = time.to_string_bytes();
        self.index = 0;
    }
}

impl Default for Clk {
    fn default() -> Clk {
        Clk::new()
    }
}

/// The card as an IO device over its slot's 16-byte DEVSEL range; only the
/// low nibble is decoded, so the same device works in any slot.
impl Device for Clk {
    fn read(&mut self, addr: u16, _cycles: u64) -> u8 {
        match addr & 0x0f {
            // Data port: next string byte, auto-incrementing. Once the string
            // is spent this falls through to $00 (every real byte has the high
            // bit set, so the firmware reads that as its terminator).
            0x1 if self.index < STRING_LEN => {
                let b = self.buf[self.index];
                self.index += 1;
                b
            }
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, _b: u8, _cycles: u64) {
        // Latch port: sample the clock and rewind to the start of the string.
        if addr & 0x0f == 0x0 {
            self.latch();
        }
    }
}

/// The latched time string mid-read (notes/STATE.md §5) — a suspended
/// ProDOS clock read resumes where it left off; the *next* latch samples
/// the host clock, which is "now" by definition. The `fixed` test aid is
/// not written.
impl ewm_core::state::Persist for Clk {
    fn save(&self, w: &mut ewm_core::state::Writer) {
        w.put_u16(self.index as u16);
        w.put_bytes(&self.buf);
    }

    fn restore(&mut self, r: &mut ewm_core::state::Reader) -> ewm_core::state::Result<()> {
        self.index = (r.get_u16()? as usize).min(STRING_LEN);
        self.buf.copy_from_slice(r.get_bytes(STRING_LEN)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator reproduces the original hand-assembled slot-1 firmware
    /// byte-for-byte (the literal below is the pre-generator static).
    #[test]
    fn slot_1_rom_matches_the_original_bytes() {
        let golden: [u8; 33] = [
            0x08, 0x78, 0x28, 0x18, 0x58, 0xea, 0x70, 0x08, 0x4c, 0x10, 0xc1, 0x60, 0xea, 0xea,
            0xea, 0xea, 0x8d, 0x90, 0xc0, 0xa0, 0x00, 0xad, 0x91, 0xc0, 0xf0, 0x06, 0x99, 0x00,
            0x02, 0xc8, 0xd0, 0xf5, 0x60,
        ];
        let rom = clk_rom(1);
        assert_eq!(rom[..golden.len()], golden);
        assert!(rom[golden.len()..].iter().all(|&b| b == 0));
        assert_eq!(clk_read_entry(1), 0xc108);
    }

    /// A moved card patches exactly the page and port operands.
    #[test]
    fn moved_rom_patches_the_slot_operands() {
        let rom = clk_rom(2);
        assert_eq!(rom[0x0a], 0xc2); // JMP $C210
        assert_eq!(rom[0x11], 0xa0); // STA $C0A0
        assert_eq!(rom[0x16], 0xa1); // LDA $C0A1
        assert_eq!(rom[0x01], 0x78, "must not look bootable");
        assert_eq!(clk_read_entry(2), 0xc208);
        // Everything else identical to the slot-1 image.
        let base = clk_rom(1);
        for (i, (&a, &b)) in base.iter().zip(rom.iter()).enumerate() {
            if ![0x0a, 0x11, 0x16].contains(&i) {
                assert_eq!(a, b, "unexpected difference at offset {i:#04x}");
            }
        }
    }

    fn sample() -> ClockTime {
        // Monday 2026-07-06 10:30:59.
        ClockTime {
            month: 7,
            weekday: 1,
            day: 6,
            hour: 10,
            minute: 30,
            second: 59,
        }
    }

    #[test]
    fn string_format_matches_the_card() {
        let s = sample().to_string_bytes();
        assert_eq!(
            s,
            [
                0xb0, 0xb7, 0xac, // 07,
                0xb0, 0xb1, 0xac, // 01, (Monday)
                0xb0, 0xb6, 0xac, // 06,
                0xb1, 0xb0, 0xac, // 10,
                0xb3, 0xb0, 0xac, // 30,
                0xb5, 0xb9, // 59
                0x8d, // CR
            ]
        );
    }

    #[test]
    fn every_byte_has_the_high_bit_set() {
        // The firmware treats a $00 as the terminator, so no real byte may be
        // zero — high-bit ASCII guarantees it.
        for b in sample().to_string_bytes() {
            assert_ne!(b, 0);
            assert_eq!(b & 0x80, 0x80);
        }
    }

    #[test]
    fn fields_are_zero_padded_to_two_digits() {
        let s = ClockTime {
            month: 1,
            weekday: 0,
            day: 2,
            hour: 3,
            minute: 4,
            second: 5,
        }
        .to_string_bytes();
        assert_eq!(&s[0..2], &[0xb0, 0xb1]); // 01
        assert_eq!(&s[3..5], &[0xb0, 0xb0]); // 00 (Sunday)
        assert_eq!(&s[6..8], &[0xb0, 0xb2]); // 02
        assert_eq!(&s[9..11], &[0xb0, 0xb3]); // 03
        assert_eq!(&s[12..14], &[0xb0, 0xb4]); // 04
    }

    #[test]
    fn ports_serve_the_string_then_zero() {
        let mut clk = Clk::new();
        clk.set_fixed_time(sample());
        clk.write(0xc090, 0, 0); // latch
        let expected = sample().to_string_bytes();
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(clk.read(0xc091, 0), *want, "byte {i}");
        }
        // Exhausted: sticks at zero until the next latch.
        assert_eq!(clk.read(0xc091, 0), 0);
        assert_eq!(clk.read(0xc091, 0), 0);
        // Re-latching restarts the string.
        clk.write(0xc090, 0, 0);
        assert_eq!(clk.read(0xc091, 0), expected[0]);
    }

    #[test]
    fn reads_before_a_latch_are_zero() {
        let mut clk = Clk::new();
        assert_eq!(clk.read(0xc091, 0), 0);
    }
}
