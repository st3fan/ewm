//! Disassembly and state formatting. Port of `fmt.c`, quirks included: the
//! `0x9c` (STZ abs) case prints `JMP`, and BBR/BBS branch targets are printed
//! relative to `pc + 2` — both exactly as the C does.

use crate::cpu::Cpu;

/// Port of `cpu_format_state`: registers, packed status byte, and the
/// `NV-BDIZC` flag string.
pub fn format_state(cpu: &Cpu) -> String {
    format!(
        "A={:02X} X={:02X} Y={:02X} S={:02X} SP={:02X} {}{}{}{}{}{}{}{}",
        cpu.a,
        cpu.x,
        cpu.y,
        cpu.status(),
        cpu.sp,
        if cpu.n != 0 { 'N' } else { '-' },
        if cpu.v != 0 { 'V' } else { '-' },
        '-',
        if cpu.b != 0 { 'B' } else { '-' },
        if cpu.d != 0 { 'D' } else { '-' },
        if cpu.i != 0 { 'I' } else { '-' },
        if cpu.z != 0 { 'Z' } else { '-' },
        if cpu.c != 0 { 'C' } else { '-' },
    )
}

/// Port of `cpu_format_stack`: the used portion of the stack as
/// `[xx xx ...]`, empty when sp is 0xff.
pub fn format_stack(cpu: &mut Cpu) -> String {
    let mut buffer = String::new();
    if cpu.sp != 0xff {
        buffer.push('[');
        let mut sp = cpu.sp as u16;
        while sp != 0xff {
            if sp != cpu.sp as u16 {
                buffer.push(' ');
            }
            buffer.push_str(&format!("{:02X}", cpu.mem.read(0x0100 + sp + 1)));
            sp += 1;
        }
        buffer.push(']');
    }
    buffer
}

/// Port of `cpu_format_instruction`: disassemble the instruction at the
/// current pc. The layout (including `%-4s` mnemonic padding) matches the C
/// sprintf formats byte for byte.
pub fn format_instruction(cpu: &mut Cpu) -> String {
    let pc = cpu.pc;
    let opcode = cpu.mem.read(pc);
    let instructions = cpu.instructions;
    let i = &instructions[opcode as usize];
    let name = i.name;

    // (The C code checks for a NULL handler here, but the tables a Cpu ever
    // holds are fully backfilled, so that branch is unreachable.)

    /* Single byte instructions */
    if i.bytes == 1 {
        format!("{:<4}", name)
    }
    /* 65C02 ADC, AND, CMP, EOR, LDA, ORA, SBC, STA (zp) */
    else if (opcode & 0b0001_1111) == 0b0001_0010 {
        format!("{:<4} ${:02X}", name, cpu.mem.read(pc.wrapping_add(1)))
    }
    /* 65C02 RMB / SMB */
    else if (opcode & 0b0000_1111) == 0b0000_0111 {
        format!(
            "{}{} ${:02X}",
            if (opcode & 0b1000_0000) == 0 {
                "RMB"
            } else {
                "SMB"
            },
            (opcode & 0b0111_0000) >> 4,
            cpu.mem.read(pc.wrapping_add(1))
        )
    }
    /* 65C02 BBR / BBS */
    else if (opcode & 0b0000_1111) == 0b0000_1111 {
        let offset = cpu.mem.read(pc.wrapping_add(2)) as i8;
        format!(
            "{}{} ${:02X},${:04X}",
            if (opcode & 0b1000_0000) == 0 {
                "BBR"
            } else {
                "BBS"
            },
            (opcode & 0b0111_0000) >> 4,
            cpu.mem.read(pc.wrapping_add(1)),
            pc.wrapping_add(2).wrapping_add(offset as u16)
        )
    }
    /* 65C02 JMP (ABS,X) */
    else if opcode == 0x7c {
        format!("JMP (${:04X},X)", cpu.mem.read_word(pc.wrapping_add(1)))
    }
    /* 65C02 BRA */
    else if opcode == 0x80 {
        let offset = cpu.mem.read(pc.wrapping_add(1)) as i8;
        format!(
            "BRA ${:04X}",
            pc.wrapping_add(2).wrapping_add(offset as u16)
        )
    }
    /* 65C02 STZ ABS (prints JMP, as fmt.c does) */
    else if opcode == 0x9c {
        format!("JMP  ${:04X}", cpu.mem.read_word(pc.wrapping_add(1)))
    }
    /* 65C02 TRB ZP */
    else if opcode == 0x14 {
        format!("TRB  ${:02X}", cpu.mem.read(pc.wrapping_add(1)))
    }
    /* 65C02 TRB ABS */
    else if opcode == 0x1c {
        format!("TRB  ${:04X}", cpu.mem.read_word(pc.wrapping_add(1)))
    }
    /* 65C02 TSB ZP */
    else if opcode == 0x04 {
        format!("TSB  ${:02X}", cpu.mem.read(pc.wrapping_add(1)))
    }
    /* 65C02 TSB ABS */
    else if opcode == 0x0c {
        format!("TSB  ${:04X}", cpu.mem.read_word(pc.wrapping_add(1)))
    }
    /* JSR is the only exception */
    else if opcode == 0x20 {
        format!("{:<4} ${:04X}", name, cpu.mem.read_word(pc.wrapping_add(1)))
    }
    /* Branches */
    else if (opcode & 0b0001_1111) == 0b0001_0000 {
        let offset = cpu.mem.read(pc.wrapping_add(1)) as i8;
        let addr = pc.wrapping_add(2).wrapping_add(offset as u16);
        format!("{:<4} ${:04X}", name, addr)
    } else if (opcode & 0b0000_0011) == 0b0000_0001 {
        match (opcode & 0b0001_1100) >> 2 {
            0b000 => format!("{:<4} (${:02X},X)", name, cpu.mem.read(pc.wrapping_add(1))),
            0b001 => format!("{:<4} ${:02X}", name, cpu.mem.read(pc.wrapping_add(1))),
            0b010 => format!("{:<4} #${:02X}", name, cpu.mem.read(pc.wrapping_add(1))),
            0b011 => format!(
                "{:<4} ${:02X}{:02X}",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
            0b100 => format!("{:<4} (${:02X}),Y", name, cpu.mem.read(pc.wrapping_add(1))),
            0b101 => format!("{:<4} ${:02X},X", name, cpu.mem.read(pc.wrapping_add(1))),
            0b110 => format!(
                "{:<4} ${:02X}{:02X},Y",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
            _ => format!(
                "{:<4} ${:02X}{:02X},X",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
        }
    } else if (opcode & 0b0000_0011) == 0b0000_0010 {
        match (opcode & 0b0001_1100) >> 2 {
            0b000 => format!("{:<4} #${:02X}", name, cpu.mem.read(pc.wrapping_add(1))),
            0b001 => format!("{:<4} ${:02X}", name, cpu.mem.read(pc.wrapping_add(1))),
            0b010 => format!("{:<4}", name),
            0b011 => format!(
                "{:<4} ${:02X}{:02X}",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
            0b101 => format!("{:<4} ${:02X},X", name, cpu.mem.read(pc.wrapping_add(1))),
            0b111 => format!(
                "{:<4} ${:02X}{:02X},X",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
            _ => String::new(),
        }
    } else if (opcode & 0b0000_0011) == 0b0000_0000 {
        match (opcode & 0b0001_1100) >> 2 {
            0b000 => format!("{:<4} #${:02X}", name, cpu.mem.read(pc.wrapping_add(1))),
            0b001 => format!("{:<4} ${:02X}", name, cpu.mem.read(pc.wrapping_add(1))),
            0b011 => format!(
                "{:<4} ${:02X}{:02X}",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
            0b101 => format!("{:<4} ${:02X},X", name, cpu.mem.read(pc.wrapping_add(1))),
            0b111 => format!(
                "{:<4} ${:02X}{:02X},X",
                name,
                cpu.mem.read(pc.wrapping_add(2)),
                cpu.mem.read(pc.wrapping_add(1))
            ),
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Model;
    use crate::mem::Memory;

    fn setup(model: Model, code: &[u8]) -> Cpu {
        let mut cpu = Cpu::new(model, Memory::new(0x10000));
        cpu.mem.load(0x0400, code);
        cpu.pc = 0x0400;
        cpu
    }

    #[test]
    fn format_state_matches_c_layout() {
        let mut cpu = Cpu::new(Model::M6502, Memory::new(0x10000));
        cpu.a = 0xde;
        cpu.x = 0xad;
        cpu.y = 0xbe;
        cpu.sp = 0xef;
        cpu.set_status(0x00);
        cpu.n = 0x80;
        cpu.c = 1;
        assert_eq!(format_state(&cpu), "A=DE X=AD Y=BE S=B1 SP=EF N------C");
    }

    #[test]
    fn format_stack_lists_pushed_bytes() {
        let mut cpu = Cpu::new(Model::M6502, Memory::new(0x10000));
        cpu.sp = 0xff;
        assert_eq!(format_stack(&mut cpu), "");
        cpu.push_byte(0x12);
        cpu.push_byte(0x34);
        assert_eq!(format_stack(&mut cpu), "[34 12]");
    }

    #[test]
    fn format_instruction_6502() {
        let cases: &[(&[u8], &str)] = &[
            (&[0xea], "NOP "),                     // implied pads to 4
            (&[0xa9, 0x42], "LDA  #$42"),          // immediate
            (&[0xa5, 0x10], "LDA  $10"),           // zero page
            (&[0xbd, 0x34, 0x12], "LDA  $1234,X"), // absolute,X
            (&[0xb1, 0x20], "LDA  ($20),Y"),       // (zp),Y
            (&[0xa1, 0x20], "LDA  ($20,X)"),       // (zp,X)
            (&[0x20, 0x34, 0x12], "JSR  $1234"),   // JSR
            (&[0x4c, 0x34, 0x12], "JMP  $1234"),   // JMP abs
        ];
        for (code, expected) in cases {
            let mut cpu = setup(Model::M6502, code);
            assert_eq!(&format_instruction(&mut cpu), expected);
        }
        // Branch target is pc + 2 + offset.
        let mut cpu = setup(Model::M6502, &[0xd0, 0xfe]);
        assert_eq!(format_instruction(&mut cpu), "BNE  $0400");
    }

    #[test]
    fn format_instruction_65c02() {
        let cases: &[(&[u8], &str)] = &[
            (&[0xb2, 0x20], "LDA  $20"),             // (zp) mode
            (&[0x07, 0x10], "RMB0 $10"),             // RMB
            (&[0x97, 0x10], "SMB1 $10"),             // SMB
            (&[0x0f, 0x10, 0x02], "BBR0 $10,$0404"), // BBR: target pc+2+off
            (&[0x8f, 0x10, 0x02], "BBS0 $10,$0404"), // BBS
            (&[0x7c, 0x34, 0x12], "JMP ($1234,X)"),  // JMP (abs,X)
            (&[0x80, 0x02], "BRA $0404"),            // BRA
            (&[0x9c, 0x34, 0x12], "JMP  $1234"),     // STZ abs prints JMP (C bug)
            (&[0x14, 0x10], "TRB  $10"),             // TRB zp
            (&[0x0c, 0x34, 0x12], "TSB  $1234"),     // TSB abs
            (&[0xda], "PHX "),                       // implied
        ];
        for (code, expected) in cases {
            let mut cpu = setup(Model::M65C02, code);
            assert_eq!(&format_instruction(&mut cpu), expected);
        }
    }
}
