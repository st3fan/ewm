//! Instruction dispatch table and handlers. Port of the 6502 half of `ins.c`,
//! with the addressing-mode and read-modify-write helpers from `mem.c` moved
//! in beside their only callers. Handlers are ported in `ins.c` source order
//! so diffs against the C code are positional.

use crate::bus::Bus;
use crate::cpu::{Cpu, Model};

/// The type-safe version of C's arity-cast `void *handler`: dispatch selects
/// the variant matching the instruction's operand size.
#[derive(Clone, Copy)]
pub enum Handler {
    Implied(fn(&mut Cpu, &mut dyn Bus)),
    Byte(fn(&mut Cpu, &mut dyn Bus, u8)),
    Word(fn(&mut Cpu, &mut dyn Bus, u16)),
}

#[derive(Clone, Copy)]
pub struct Instruction {
    pub name: &'static str,
    pub opcode: u8,
    pub bytes: u8,
    pub cycles: u8,
    pub handler: Handler,
}

// Addressing-mode helpers, from mem.c. The index arithmetic is ported
// expression-for-expression: zpgx/zpgy and indx wrap within the zero page,
// but indy does *not* wrap when reading the pointer high byte (C integer
// promotion makes ($FF),Y read its pointer high byte from $0100).

fn mem_get_byte_abs(bus: &mut dyn Bus, addr: u16) -> u8 {
    bus.read(addr)
}

fn mem_get_byte_absx(cpu: &Cpu, bus: &mut dyn Bus, addr: u16) -> u8 {
    bus.read(addr.wrapping_add(cpu.x as u16))
}

fn mem_get_byte_absy(cpu: &Cpu, bus: &mut dyn Bus, addr: u16) -> u8 {
    bus.read(addr.wrapping_add(cpu.y as u16))
}

fn mem_get_byte_zpg(bus: &mut dyn Bus, addr: u8) -> u8 {
    bus.read(addr as u16)
}

fn mem_get_byte_zpgx(cpu: &Cpu, bus: &mut dyn Bus, addr: u8) -> u8 {
    bus.read((addr as u16 + cpu.x as u16) & 0x00ff)
}

fn mem_get_byte_zpgy(cpu: &Cpu, bus: &mut dyn Bus, addr: u8) -> u8 {
    bus.read((addr as u16 + cpu.y as u16) & 0x00ff)
}

fn indx_addr(cpu: &Cpu, bus: &mut dyn Bus, addr: u8) -> u16 {
    let hi = bus.read((addr as u16 + 1 + cpu.x as u16) & 0x00ff);
    let lo = bus.read((addr as u16 + cpu.x as u16) & 0x00ff);
    ((hi as u16) << 8) | lo as u16
}

fn indy_addr(cpu: &Cpu, bus: &mut dyn Bus, addr: u8) -> u16 {
    // No zero-page wrap on addr + 1, matching mem_get_byte_indy in mem.c.
    let hi = bus.read(addr as u16 + 1);
    let lo = bus.read(addr as u16);
    (((hi as u16) << 8) | lo as u16).wrapping_add(cpu.y as u16)
}

fn ind_addr(bus: &mut dyn Bus, addr: u8) -> u16 {
    // No zero-page wrap on addr + 1, matching mem_get_byte_ind in mem.c.
    let hi = bus.read(addr as u16 + 1);
    let lo = bus.read(addr as u16);
    ((hi as u16) << 8) | lo as u16
}

fn mem_get_byte_indx(cpu: &Cpu, bus: &mut dyn Bus, addr: u8) -> u8 {
    let a = indx_addr(cpu, bus, addr);
    bus.read(a)
}

fn mem_get_byte_indy(cpu: &Cpu, bus: &mut dyn Bus, addr: u8) -> u8 {
    let a = indy_addr(cpu, bus, addr);
    bus.read(a)
}

fn mem_get_byte_ind(bus: &mut dyn Bus, addr: u8) -> u8 {
    let a = ind_addr(bus, addr);
    bus.read(a)
}

fn mem_set_byte_zpg(bus: &mut dyn Bus, addr: u8, v: u8) {
    bus.write(addr as u16, v);
}

fn mem_set_byte_zpgx(cpu: &Cpu, bus: &mut dyn Bus, addr: u8, v: u8) {
    bus.write((addr as u16 + cpu.x as u16) & 0x00ff, v);
}

fn mem_set_byte_zpgy(cpu: &Cpu, bus: &mut dyn Bus, addr: u8, v: u8) {
    bus.write((addr as u16 + cpu.y as u16) & 0x00ff, v);
}

fn mem_set_byte_abs(bus: &mut dyn Bus, addr: u16, v: u8) {
    bus.write(addr, v);
}

fn mem_set_byte_absx(cpu: &Cpu, bus: &mut dyn Bus, addr: u16, v: u8) {
    bus.write(addr.wrapping_add(cpu.x as u16), v);
}

fn mem_set_byte_absy(cpu: &Cpu, bus: &mut dyn Bus, addr: u16, v: u8) {
    bus.write(addr.wrapping_add(cpu.y as u16), v);
}

fn mem_set_byte_indx(cpu: &Cpu, bus: &mut dyn Bus, addr: u8, v: u8) {
    let a = indx_addr(cpu, bus, addr);
    bus.write(a, v);
}

fn mem_set_byte_indy(cpu: &Cpu, bus: &mut dyn Bus, addr: u8, v: u8) {
    let a = indy_addr(cpu, bus, addr);
    bus.write(a, v);
}

fn mem_set_byte_ind(bus: &mut dyn Bus, addr: u8, v: u8) {
    let a = ind_addr(bus, addr);
    bus.write(a, v);
}

// Read-modify-write helpers, from mem.c (mem_mod_byte_*). Only the variants
// the 6502 table uses are ported; more arrive with the 65C02 in Phase 2.

type ModOp = fn(&mut Cpu, u8) -> u8;

fn mem_mod_byte_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, addr: u8, op: ModOp) {
    let b = mem_get_byte_zpg(bus, addr);
    let v = op(cpu, b);
    mem_set_byte_zpg(bus, addr, v);
}

fn mem_mod_byte_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, addr: u8, op: ModOp) {
    let b = mem_get_byte_zpgx(cpu, bus, addr);
    let v = op(cpu, b);
    mem_set_byte_zpgx(cpu, bus, addr, v);
}

fn mem_mod_byte_abs(cpu: &mut Cpu, bus: &mut dyn Bus, addr: u16, op: ModOp) {
    let b = mem_get_byte_abs(bus, addr);
    let v = op(cpu, b);
    mem_set_byte_abs(bus, addr, v);
}

fn mem_mod_byte_absx(cpu: &mut Cpu, bus: &mut dyn Bus, addr: u16, op: ModOp) {
    let b = mem_get_byte_absx(cpu, bus, addr);
    let v = op(cpu, b);
    mem_set_byte_absx(cpu, bus, addr, v);
}

fn update_zn(cpu: &mut Cpu, v: u8) {
    cpu.z = (v == 0x00) as u8;
    cpu.n = v & 0x80;
}

// EWM_CPU_MODEL_6502

/* ADC */

fn adc(cpu: &mut Cpu, m: u8) {
    let c: u8 = if cpu.c != 0 { 1 } else { 0 };
    if cpu.d != 0 {
        let mut cb = 0u8;

        let mut low = (cpu.a & 0x0f) + (m & 0x0f) + c;
        if low > 9 {
            low += 6;
        }
        if low > 15 {
            cb = 1;
        }

        let mut high = (cpu.a >> 4) + (m >> 4) + cb;
        if high > 9 {
            high += 6;
        }
        let r = (low & 0x0f) | ((high << 4) & 0xf0);

        cpu.c = (high > 15) as u8;
        cpu.z = (r == 0) as u8;
        cpu.n = r & 0b1000_0000; // TODO Only on 6502? Does the 6502 test still pass?
        cpu.v = 0;

        cpu.a = r;
    } else {
        let t = cpu.a as u16 + m as u16 + c as u16;
        let r = t as u8;
        cpu.c = ((t & 0x0100) != 0) as u8;
        cpu.v = (cpu.a ^ r) & (m ^ r) & 0x80;
        cpu.a = r;
        update_zn(cpu, cpu.a);
    }
}

fn adc_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    adc(cpu, oper);
}

fn adc_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    adc(cpu, m);
}

fn adc_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    adc(cpu, m);
}

fn adc_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    adc(cpu, m);
}

fn adc_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    adc(cpu, m);
}

fn adc_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absy(cpu, bus, oper);
    adc(cpu, m);
}

fn adc_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indx(cpu, bus, oper);
    adc(cpu, m);
}

fn adc_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indy(cpu, bus, oper);
    adc(cpu, m);
}

/* AND */

fn and(cpu: &mut Cpu, m: u8) {
    cpu.a &= m;
    update_zn(cpu, cpu.a);
}

fn and_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    and(cpu, oper);
}

fn and_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    and(cpu, m);
}

fn and_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    and(cpu, m);
}

fn and_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    and(cpu, m);
}

fn and_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    and(cpu, m);
}

fn and_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absy(cpu, bus, oper);
    and(cpu, m);
}

fn and_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indx(cpu, bus, oper);
    and(cpu, m);
}

fn and_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indy(cpu, bus, oper);
    and(cpu, m);
}

/* ASL */

fn asl(cpu: &mut Cpu, b: u8) -> u8 {
    cpu.c = b & 0x80;
    let b = b << 1;
    cpu.n = b & 0x80;
    cpu.z = (b == 0) as u8;
    b
}

fn asl_acc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = asl(cpu, cpu.a);
}

fn asl_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpg(cpu, bus, oper, asl);
}

fn asl_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpgx(cpu, bus, oper, asl);
}

fn asl_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_abs(cpu, bus, oper, asl);
}

fn asl_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_absx(cpu, bus, oper, asl);
}

/* BIT */

fn bit(cpu: &mut Cpu, m: u8) {
    let t = cpu.a & m;
    cpu.n = m & 0x80;
    cpu.v = m & 0x40;
    cpu.z = (t == 0) as u8;
}

fn bit_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    bit(cpu, m);
}

fn bit_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    bit(cpu, m);
}

/* Bxx Branches */

fn bcc(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.c == 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn bcs(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.c != 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn beq(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.z != 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn bmi(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.n != 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn bne(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.z == 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn bpl(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.n == 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn bvc(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.v == 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

fn bvs(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    if cpu.v != 0 {
        cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
    }
}

/* BRK */

fn brk(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.b = 1;
    if cpu.model == Model::M65C02 {
        cpu.d = 0;
    }
    let _ = cpu.irq(bus);
}

/* CLx */

fn clc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.c = 0;
}

fn cld(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.d = 0;
}

fn cli(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.i = 0;
}

fn clv(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.v = 0;
}

/* CMP */

fn cmp(cpu: &mut Cpu, m: u8) {
    let t = cpu.a.wrapping_sub(m);
    cpu.c = (cpu.a >= m) as u8;
    cpu.n = t & 0x80;
    cpu.z = (t == 0) as u8;
}

fn cmp_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cmp(cpu, oper);
}

fn cmp_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    cmp(cpu, m);
}

fn cmp_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    cmp(cpu, m);
}

fn cmp_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    cmp(cpu, m);
}

fn cmp_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    cmp(cpu, m);
}

fn cmp_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absy(cpu, bus, oper);
    cmp(cpu, m);
}

fn cmp_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indx(cpu, bus, oper);
    cmp(cpu, m);
}

fn cmp_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indy(cpu, bus, oper);
    cmp(cpu, m);
}

/* CPX */

fn cpx(cpu: &mut Cpu, m: u8) {
    let t = cpu.x.wrapping_sub(m);
    cpu.c = (cpu.x >= m) as u8;
    update_zn(cpu, t);
}

fn cpx_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cpx(cpu, oper);
}

fn cpx_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    cpx(cpu, m);
}

fn cpx_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    cpx(cpu, m);
}

/* CPY */

fn cpy(cpu: &mut Cpu, m: u8) {
    let t = cpu.y.wrapping_sub(m);
    cpu.c = (cpu.y >= m) as u8;
    update_zn(cpu, t);
}

fn cpy_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cpy(cpu, oper);
}

fn cpy_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    cpy(cpu, m);
}

fn cpy_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    cpy(cpu, m);
}

/* DEx */

fn dec(cpu: &mut Cpu, b: u8) -> u8 {
    let t = b.wrapping_sub(1);
    update_zn(cpu, t);
    t
}

fn dec_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpg(cpu, bus, oper, dec);
}

fn dec_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpgx(cpu, bus, oper, dec);
}

fn dec_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_abs(cpu, bus, oper, dec);
}

fn dec_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_absx(cpu, bus, oper, dec);
}

fn dex(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.x = cpu.x.wrapping_sub(1);
    update_zn(cpu, cpu.x);
}

fn dey(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.y = cpu.y.wrapping_sub(1);
    update_zn(cpu, cpu.y);
}

/* EOR */

fn eor(cpu: &mut Cpu, m: u8) {
    cpu.a ^= m;
    update_zn(cpu, cpu.a);
}

fn eor_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    eor(cpu, oper);
}

fn eor_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    eor(cpu, m);
}

fn eor_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    eor(cpu, m);
}

fn eor_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    eor(cpu, m);
}

fn eor_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    eor(cpu, m);
}

fn eor_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absy(cpu, bus, oper);
    eor(cpu, m);
}

fn eor_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indx(cpu, bus, oper);
    eor(cpu, m);
}

fn eor_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indy(cpu, bus, oper);
    eor(cpu, m);
}

/* INx */

fn inc(cpu: &mut Cpu, b: u8) -> u8 {
    let t = b.wrapping_add(1);
    update_zn(cpu, t);
    t
}

fn inc_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpg(cpu, bus, oper, inc);
}

fn inc_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpgx(cpu, bus, oper, inc);
}

fn inc_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_abs(cpu, bus, oper, inc);
}

fn inc_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_absx(cpu, bus, oper, inc);
}

fn inx(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.x = cpu.x.wrapping_add(1);
    update_zn(cpu, cpu.x);
}

fn iny(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.y = cpu.y.wrapping_add(1);
    update_zn(cpu, cpu.y);
}

/* JMP */

fn jmp_abs(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u16) {
    cpu.pc = oper;
}

fn jmp_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.pc = bus.read_word(oper);
}

/* JSR */

fn jsr_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.push_word(bus, cpu.pc.wrapping_sub(1));
    cpu.pc = oper;
}

/* LDA */

fn lda_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cpu.a = oper;
    update_zn(cpu, cpu.a);
}

fn lda_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.a = mem_get_byte_zpg(bus, oper);
    update_zn(cpu, cpu.a);
}

fn lda_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.a = mem_get_byte_zpgx(cpu, bus, oper);
    update_zn(cpu, cpu.a);
}

fn lda_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.a = mem_get_byte_abs(bus, oper);
    update_zn(cpu, cpu.a);
}

fn lda_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.a = mem_get_byte_absx(cpu, bus, oper);
    update_zn(cpu, cpu.a);
}

fn lda_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.a = mem_get_byte_absy(cpu, bus, oper);
    update_zn(cpu, cpu.a);
}

fn lda_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.a = mem_get_byte_indx(cpu, bus, oper);
    update_zn(cpu, cpu.a);
}

fn lda_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.a = mem_get_byte_indy(cpu, bus, oper);
    update_zn(cpu, cpu.a);
}

/* LDX */

fn ldx_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cpu.x = oper;
    update_zn(cpu, cpu.x);
}

fn ldx_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.x = mem_get_byte_zpg(bus, oper);
    update_zn(cpu, cpu.x);
}

fn ldx_zpgy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.x = mem_get_byte_zpgy(cpu, bus, oper);
    update_zn(cpu, cpu.x);
}

fn ldx_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.x = mem_get_byte_abs(bus, oper);
    update_zn(cpu, cpu.x);
}

fn ldx_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.x = mem_get_byte_absy(cpu, bus, oper);
    update_zn(cpu, cpu.x);
}

/* LDY */

fn ldy_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cpu.y = oper;
    update_zn(cpu, cpu.y);
}

fn ldy_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.y = mem_get_byte_zpg(bus, oper);
    update_zn(cpu, cpu.y);
}

fn ldy_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.y = mem_get_byte_zpgx(cpu, bus, oper);
    update_zn(cpu, cpu.y);
}

fn ldy_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.y = mem_get_byte_abs(bus, oper);
    update_zn(cpu, cpu.y);
}

fn ldy_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.y = mem_get_byte_absx(cpu, bus, oper);
    update_zn(cpu, cpu.y);
}

/* LSR */

fn lsr(cpu: &mut Cpu, b: u8) -> u8 {
    cpu.c = b & 1;
    let b = b >> 1;
    update_zn(cpu, b);
    b
}

fn lsr_acc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = lsr(cpu, cpu.a);
}

fn lsr_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpg(cpu, bus, oper, lsr);
}

fn lsr_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpgx(cpu, bus, oper, lsr);
}

fn lsr_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_abs(cpu, bus, oper, lsr);
}

fn lsr_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_absx(cpu, bus, oper, lsr);
}

/* NOP */

fn nop(_cpu: &mut Cpu, _bus: &mut dyn Bus) {}

/* ORA */

fn ora(cpu: &mut Cpu, m: u8) {
    cpu.a |= m;
    update_zn(cpu, cpu.a);
}

fn ora_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    ora(cpu, oper);
}

fn ora_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    ora(cpu, m);
}

fn ora_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    ora(cpu, m);
}

fn ora_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    ora(cpu, m);
}

fn ora_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    ora(cpu, m);
}

fn ora_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absy(cpu, bus, oper);
    ora(cpu, m);
}

fn ora_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indx(cpu, bus, oper);
    ora(cpu, m);
}

fn ora_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indy(cpu, bus, oper);
    ora(cpu, m);
}

/* P** */

fn pha(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.push_byte(bus, cpu.a);
}

fn pla(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.a = cpu.pull_byte(bus);
    update_zn(cpu, cpu.a);
}

fn php(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.push_byte(bus, cpu.status());
}

fn plp(cpu: &mut Cpu, bus: &mut dyn Bus) {
    let s = cpu.pull_byte(bus);
    cpu.set_status(s);
}

/* ROL */

fn rol(cpu: &mut Cpu, b: u8) -> u8 {
    let carry: u8 = if cpu.c != 0 { 1 } else { 0 };
    cpu.c = b & 0x80;
    let b = (b << 1) | carry;
    update_zn(cpu, b);
    b
}

fn rol_acc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = rol(cpu, cpu.a);
}

fn rol_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpg(cpu, bus, oper, rol);
}

fn rol_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpgx(cpu, bus, oper, rol);
}

fn rol_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_abs(cpu, bus, oper, rol);
}

fn rol_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_absx(cpu, bus, oper, rol);
}

/* ROR */

fn ror(cpu: &mut Cpu, b: u8) -> u8 {
    let carry: u8 = if cpu.c != 0 { 1 } else { 0 };
    cpu.c = b & 0x01;
    let b = (b >> 1) | (carry << 7);
    update_zn(cpu, b);
    b
}

fn ror_acc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = ror(cpu, cpu.a);
}

fn ror_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpg(cpu, bus, oper, ror);
}

fn ror_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_mod_byte_zpgx(cpu, bus, oper, ror);
}

fn ror_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_abs(cpu, bus, oper, ror);
}

fn ror_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_mod_byte_absx(cpu, bus, oper, ror);
}

/* RTI */

fn rti(cpu: &mut Cpu, bus: &mut dyn Bus) {
    let s = cpu.pull_byte(bus);
    cpu.set_status(s);
    cpu.pc = cpu.pull_word(bus);
}

/* RTS */

fn rts(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.pc = cpu.pull_word(bus).wrapping_add(1);
}

/* SBC */

fn sbc(cpu: &mut Cpu, m: u8) {
    let mut c: u8 = if cpu.c != 0 { 1 } else { 0 };
    if cpu.d != 0 {
        let mut cb = 0u8;

        if c == 0 {
            c = 1;
        } else {
            c = 0;
        }

        let mut low = (cpu.a & 0x0f).wrapping_sub(m & 0x0f).wrapping_sub(c);
        if (low & 0x10) != 0 {
            low = low.wrapping_sub(6);
        }
        if (low & 0x10) != 0 {
            cb = 1;
        }

        let mut high = (cpu.a >> 4).wrapping_sub(m >> 4).wrapping_sub(cb);
        if (high & 0x10) != 0 {
            high = high.wrapping_sub(6);
        }

        let result = (low & 0x0f) | (high << 4);

        cpu.c = (high < 15) as u8;
        cpu.z = (result == 0) as u8;
        cpu.n = result & 0b1000_0000; // TODO Only on 6502? Does the 6502 test still pass?
        cpu.v = 0;

        cpu.a = result;
    } else {
        adc(cpu, m ^ 0xff);
    }
}

fn sbc_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    sbc(cpu, oper);
}

fn sbc_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpg(bus, oper);
    sbc(cpu, m);
}

fn sbc_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    sbc(cpu, m);
}

fn sbc_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_abs(bus, oper);
    sbc(cpu, m);
}

fn sbc_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    sbc(cpu, m);
}

fn sbc_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absy(cpu, bus, oper);
    sbc(cpu, m);
}

fn sbc_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indx(cpu, bus, oper);
    sbc(cpu, m);
}

fn sbc_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_indy(cpu, bus, oper);
    sbc(cpu, m);
}

/* SEx */

fn sec(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.c = 1;
}

fn sed(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.d = 1;
}

fn sei(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.i = 1;
}

/* STA */

fn sta_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpg(bus, oper, cpu.a);
}

fn sta_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpgx(cpu, bus, oper, cpu.a);
}

fn sta_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_abs(bus, oper, cpu.a);
}

fn sta_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_absx(cpu, bus, oper, cpu.a);
}

fn sta_absy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_absy(cpu, bus, oper, cpu.a);
}

fn sta_indx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_indx(cpu, bus, oper, cpu.a);
}

fn sta_indy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_indy(cpu, bus, oper, cpu.a);
}

/* STX */

fn stx_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpg(bus, oper, cpu.x);
}

fn stx_zpgy(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpgy(cpu, bus, oper, cpu.x);
}

fn stx_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_abs(bus, oper, cpu.x);
}

/* STY */

fn sty_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpg(bus, oper, cpu.y);
}

fn sty_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpgx(cpu, bus, oper, cpu.y);
}

fn sty_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_abs(bus, oper, cpu.y);
}

/* Txx */

fn tax(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.x = cpu.a;
    update_zn(cpu, cpu.x);
}

fn tay(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.y = cpu.a;
    update_zn(cpu, cpu.y);
}

fn tsx(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.x = cpu.sp;
    update_zn(cpu, cpu.x);
}

fn txa(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = cpu.x;
    update_zn(cpu, cpu.a);
}

fn txs(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.sp = cpu.x;
}

fn tya(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = cpu.y;
    update_zn(cpu, cpu.a);
}

fn unimplemented(_cpu: &mut Cpu, _bus: &mut dyn Bus) {
    // Undocumented opcodes execute as 1-byte, 2-cycle no-ops, as in ins.c.
}

/* Instruction dispatch table */

const fn ins(
    name: &'static str,
    opcode: u8,
    bytes: u8,
    cycles: u8,
    handler: Handler,
) -> Instruction {
    Instruction {
        name,
        opcode,
        bytes,
        cycles,
        handler,
    }
}

use Handler::{Byte, Implied, Word};

#[rustfmt::skip]
pub static INSTRUCTIONS_6502: [Instruction; 256] = [
    /* 0x00 */ ins("BRK", 0x00, 1, 2, Implied(brk)),
    /* 0x01 */ ins("ORA", 0x01, 2, 6, Byte(ora_indx)),
    /* 0x02 */ ins("???", 0x02, 1, 2, Implied(unimplemented)),
    /* 0x03 */ ins("???", 0x03, 1, 2, Implied(unimplemented)),
    /* 0x04 */ ins("???", 0x04, 1, 2, Implied(unimplemented)),
    /* 0x05 */ ins("ORA", 0x05, 2, 2, Byte(ora_zpg)),
    /* 0x06 */ ins("ASL", 0x06, 2, 5, Byte(asl_zpg)),
    /* 0x07 */ ins("???", 0x07, 1, 2, Implied(unimplemented)),
    /* 0x08 */ ins("PHP", 0x08, 1, 3, Implied(php)),
    /* 0x09 */ ins("ORA", 0x09, 2, 2, Byte(ora_imm)),
    /* 0x0a */ ins("ASL", 0x0a, 1, 2, Implied(asl_acc)),
    /* 0x0b */ ins("???", 0x0b, 1, 2, Implied(unimplemented)),
    /* 0x0c */ ins("???", 0x0c, 1, 2, Implied(unimplemented)),
    /* 0x0d */ ins("ORA", 0x0d, 3, 4, Word(ora_abs)),
    /* 0x0e */ ins("ASL", 0x0e, 3, 6, Word(asl_abs)),
    /* 0x0f */ ins("???", 0x0f, 1, 2, Implied(unimplemented)),
    /* 0x10 */ ins("BPL", 0x10, 2, 2, Byte(bpl)),
    /* 0x11 */ ins("ORA", 0x11, 2, 5, Byte(ora_indy)),
    /* 0x12 */ ins("???", 0x12, 1, 2, Implied(unimplemented)),
    /* 0x13 */ ins("???", 0x13, 1, 2, Implied(unimplemented)),
    /* 0x14 */ ins("???", 0x14, 1, 2, Implied(unimplemented)),
    /* 0x15 */ ins("ORA", 0x15, 2, 3, Byte(ora_zpgx)),
    /* 0x16 */ ins("ASL", 0x16, 2, 6, Byte(asl_zpgx)),
    /* 0x17 */ ins("???", 0x17, 1, 2, Implied(unimplemented)),
    /* 0x18 */ ins("CLC", 0x18, 1, 2, Implied(clc)),
    /* 0x19 */ ins("ORA", 0x19, 3, 4, Word(ora_absy)),
    /* 0x1a */ ins("???", 0x1a, 1, 2, Implied(unimplemented)),
    /* 0x1b */ ins("???", 0x1b, 1, 2, Implied(unimplemented)),
    /* 0x1c */ ins("???", 0x1c, 1, 2, Implied(unimplemented)),
    /* 0x1d */ ins("ORA", 0x1d, 3, 4, Word(ora_absx)),
    /* 0x1e */ ins("ASL", 0x1e, 3, 7, Word(asl_absx)),
    /* 0x1f */ ins("???", 0x1f, 1, 2, Implied(unimplemented)),

    /* 0x20 */ ins("JSR", 0x20, 3, 6, Word(jsr_abs)),
    /* 0x21 */ ins("AND", 0x21, 2, 6, Byte(and_indx)),
    /* 0x22 */ ins("???", 0x22, 1, 2, Implied(unimplemented)),
    /* 0x23 */ ins("???", 0x23, 1, 2, Implied(unimplemented)),
    /* 0x24 */ ins("BIT", 0x24, 2, 3, Byte(bit_zpg)),
    /* 0x25 */ ins("AND", 0x25, 2, 3, Byte(and_zpg)),
    /* 0x26 */ ins("ROL", 0x26, 2, 5, Byte(rol_zpg)),
    /* 0x27 */ ins("???", 0x27, 1, 2, Implied(unimplemented)),
    /* 0x28 */ ins("PLP", 0x28, 1, 4, Implied(plp)),
    /* 0x29 */ ins("AND", 0x29, 2, 2, Byte(and_imm)),
    /* 0x2a */ ins("ROL", 0x2a, 1, 2, Implied(rol_acc)),
    /* 0x2b */ ins("???", 0x2b, 1, 2, Implied(unimplemented)),
    /* 0x2c */ ins("BIT", 0x2c, 3, 4, Word(bit_abs)),
    /* 0x2d */ ins("AND", 0x2d, 3, 4, Word(and_abs)),
    /* 0x2e */ ins("ROL", 0x2e, 3, 6, Word(rol_abs)),
    /* 0x2f */ ins("???", 0x2f, 1, 2, Implied(unimplemented)),
    /* 0x30 */ ins("BMI", 0x30, 2, 2, Byte(bmi)),
    /* 0x31 */ ins("AND", 0x31, 2, 5, Byte(and_indy)),
    /* 0x32 */ ins("???", 0x32, 1, 2, Implied(unimplemented)),
    /* 0x33 */ ins("???", 0x33, 1, 2, Implied(unimplemented)),
    /* 0x34 */ ins("???", 0x34, 1, 2, Implied(unimplemented)),
    /* 0x35 */ ins("AND", 0x35, 2, 4, Byte(and_zpgx)),
    /* 0x36 */ ins("ROL", 0x36, 2, 6, Byte(rol_zpgx)),
    /* 0x37 */ ins("???", 0x37, 1, 2, Implied(unimplemented)),
    /* 0x38 */ ins("SEC", 0x38, 1, 2, Implied(sec)),
    /* 0x39 */ ins("AND", 0x39, 3, 4, Word(and_absy)),
    /* 0x3a */ ins("???", 0x3a, 1, 2, Implied(unimplemented)),
    /* 0x3b */ ins("???", 0x3b, 1, 2, Implied(unimplemented)),
    /* 0x3c */ ins("???", 0x3c, 1, 2, Implied(unimplemented)),
    /* 0x3d */ ins("AND", 0x3d, 3, 4, Word(and_absx)),
    /* 0x3e */ ins("ROL", 0x3e, 3, 7, Word(rol_absx)),
    /* 0x3f */ ins("???", 0x3f, 1, 2, Implied(unimplemented)),

    /* 0x40 */ ins("RTI", 0x40, 1, 6, Implied(rti)),
    /* 0x41 */ ins("EOR", 0x41, 2, 6, Byte(eor_indx)),
    /* 0x42 */ ins("???", 0x42, 1, 2, Implied(unimplemented)),
    /* 0x43 */ ins("???", 0x43, 1, 2, Implied(unimplemented)),
    /* 0x44 */ ins("???", 0x44, 1, 2, Implied(unimplemented)),
    /* 0x45 */ ins("EOR", 0x45, 2, 3, Byte(eor_zpg)),
    /* 0x46 */ ins("LSR", 0x46, 2, 5, Byte(lsr_zpg)),
    /* 0x47 */ ins("???", 0x47, 1, 2, Implied(unimplemented)),
    /* 0x48 */ ins("PHA", 0x48, 1, 3, Implied(pha)),
    /* 0x49 */ ins("EOR", 0x49, 2, 2, Byte(eor_imm)),
    /* 0x4a */ ins("LSR", 0x4a, 1, 2, Implied(lsr_acc)),
    /* 0x4b */ ins("???", 0x4b, 1, 2, Implied(unimplemented)),
    /* 0x4c */ ins("JMP", 0x4c, 3, 3, Word(jmp_abs)),
    /* 0x4d */ ins("EOR", 0x4d, 3, 4, Word(eor_abs)),
    /* 0x4e */ ins("LSR", 0x4e, 3, 6, Word(lsr_abs)),
    /* 0x4f */ ins("???", 0x4f, 1, 2, Implied(unimplemented)),
    /* 0x50 */ ins("BVC", 0x50, 2, 2, Byte(bvc)),
    /* 0x51 */ ins("EOR", 0x51, 2, 5, Byte(eor_indy)),
    /* 0x52 */ ins("???", 0x52, 1, 2, Implied(unimplemented)),
    /* 0x53 */ ins("???", 0x53, 1, 2, Implied(unimplemented)),
    /* 0x54 */ ins("???", 0x54, 1, 2, Implied(unimplemented)),
    /* 0x55 */ ins("EOR", 0x55, 2, 4, Byte(eor_zpgx)),
    /* 0x56 */ ins("LSR", 0x56, 2, 6, Byte(lsr_zpgx)),
    /* 0x57 */ ins("???", 0x57, 1, 2, Implied(unimplemented)),
    /* 0x58 */ ins("CLI", 0x58, 1, 2, Implied(cli)),
    /* 0x59 */ ins("EOR", 0x59, 3, 4, Word(eor_absy)),
    /* 0x5a */ ins("???", 0x5a, 1, 2, Implied(unimplemented)),
    /* 0x5b */ ins("???", 0x5b, 1, 2, Implied(unimplemented)),
    /* 0x5c */ ins("???", 0x5c, 1, 2, Implied(unimplemented)),
    /* 0x5d */ ins("EOR", 0x5d, 3, 4, Word(eor_absx)),
    /* 0x5e */ ins("LSR", 0x5e, 3, 7, Word(lsr_absx)),
    /* 0x5f */ ins("???", 0x5f, 1, 2, Implied(unimplemented)),

    /* 0x60 */ ins("RTS", 0x60, 1, 6, Implied(rts)),
    /* 0x61 */ ins("ADC", 0x61, 2, 6, Byte(adc_indx)),
    /* 0x62 */ ins("???", 0x62, 1, 2, Implied(unimplemented)),
    /* 0x63 */ ins("???", 0x63, 1, 2, Implied(unimplemented)),
    /* 0x64 */ ins("???", 0x64, 1, 2, Implied(unimplemented)),
    /* 0x65 */ ins("ADC", 0x65, 2, 3, Byte(adc_zpg)),
    /* 0x66 */ ins("ROR", 0x66, 2, 5, Byte(ror_zpg)),
    /* 0x67 */ ins("???", 0x67, 1, 2, Implied(unimplemented)),
    /* 0x68 */ ins("PLA", 0x68, 1, 4, Implied(pla)),
    /* 0x69 */ ins("ADC", 0x69, 2, 2, Byte(adc_imm)),
    /* 0x6a */ ins("ROR", 0x6a, 1, 2, Implied(ror_acc)),
    /* 0x6b */ ins("???", 0x6b, 1, 2, Implied(unimplemented)),
    /* 0x6c */ ins("JMP", 0x6c, 3, 5, Word(jmp_ind)),
    /* 0x6d */ ins("ADC", 0x6d, 3, 4, Word(adc_abs)),
    /* 0x6e */ ins("ROR", 0x6e, 3, 6, Word(ror_abs)),
    /* 0x6f */ ins("???", 0x6f, 1, 2, Implied(unimplemented)),
    /* 0x70 */ ins("BVS", 0x70, 2, 2, Byte(bvs)),
    /* 0x71 */ ins("ADC", 0x71, 2, 5, Byte(adc_indy)),
    /* 0x72 */ ins("???", 0x72, 1, 2, Implied(unimplemented)),
    /* 0x73 */ ins("???", 0x73, 1, 2, Implied(unimplemented)),
    /* 0x74 */ ins("???", 0x74, 1, 2, Implied(unimplemented)),
    /* 0x75 */ ins("ADC", 0x75, 2, 4, Byte(adc_zpgx)),
    /* 0x76 */ ins("ROR", 0x76, 2, 6, Byte(ror_zpgx)),
    /* 0x77 */ ins("???", 0x77, 1, 2, Implied(unimplemented)),
    /* 0x78 */ ins("SEI", 0x78, 1, 2, Implied(sei)),
    /* 0x79 */ ins("ADC", 0x79, 3, 4, Word(adc_absy)),
    /* 0x7a */ ins("???", 0x7a, 1, 2, Implied(unimplemented)),
    /* 0x7b */ ins("???", 0x7b, 1, 2, Implied(unimplemented)),
    /* 0x7c */ ins("???", 0x7c, 1, 2, Implied(unimplemented)),
    /* 0x7d */ ins("ADC", 0x7d, 3, 4, Word(adc_absx)),
    /* 0x7e */ ins("ROR", 0x7e, 3, 7, Word(ror_absx)),
    /* 0x7f */ ins("???", 0x7f, 1, 2, Implied(unimplemented)),

    /* 0x80 */ ins("???", 0x80, 1, 2, Implied(unimplemented)),
    /* 0x81 */ ins("STA", 0x81, 2, 6, Byte(sta_indx)),
    /* 0x82 */ ins("???", 0x82, 1, 2, Implied(unimplemented)),
    /* 0x83 */ ins("???", 0x83, 1, 2, Implied(unimplemented)),
    /* 0x84 */ ins("STY", 0x84, 2, 3, Byte(sty_zpg)),
    /* 0x85 */ ins("STA", 0x85, 2, 3, Byte(sta_zpg)),
    /* 0x86 */ ins("STX", 0x86, 2, 3, Byte(stx_zpg)),
    /* 0x87 */ ins("???", 0x87, 1, 2, Implied(unimplemented)),
    /* 0x88 */ ins("DEY", 0x88, 1, 2, Implied(dey)),
    /* 0x89 */ ins("???", 0x89, 1, 2, Implied(unimplemented)),
    /* 0x8a */ ins("TXA", 0x8a, 1, 2, Implied(txa)),
    /* 0x8b */ ins("???", 0x8b, 1, 2, Implied(unimplemented)),
    /* 0x8c */ ins("STY", 0x8c, 3, 4, Word(sty_abs)),
    /* 0x8d */ ins("STA", 0x8d, 3, 4, Word(sta_abs)),
    /* 0x8e */ ins("STX", 0x8e, 3, 4, Word(stx_abs)),
    /* 0x8f */ ins("???", 0x8f, 1, 2, Implied(unimplemented)),
    /* 0x90 */ ins("BCC", 0x90, 2, 2, Byte(bcc)),
    /* 0x91 */ ins("STA", 0x91, 2, 6, Byte(sta_indy)),
    /* 0x92 */ ins("???", 0x92, 1, 2, Implied(unimplemented)),
    /* 0x93 */ ins("???", 0x93, 1, 2, Implied(unimplemented)),
    /* 0x94 */ ins("STY", 0x94, 2, 4, Byte(sty_zpgx)),
    /* 0x95 */ ins("STA", 0x95, 2, 4, Byte(sta_zpgx)),
    /* 0x96 */ ins("STX", 0x96, 2, 4, Byte(stx_zpgy)),
    /* 0x97 */ ins("???", 0x97, 1, 2, Implied(unimplemented)),
    /* 0x98 */ ins("TYA", 0x98, 1, 2, Implied(tya)),
    /* 0x99 */ ins("STA", 0x99, 3, 5, Word(sta_absy)),
    /* 0x9a */ ins("TXS", 0x9a, 1, 2, Implied(txs)),
    /* 0x9b */ ins("???", 0x9b, 1, 2, Implied(unimplemented)),
    /* 0x9c */ ins("???", 0x9c, 1, 2, Implied(unimplemented)),
    /* 0x9d */ ins("STA", 0x9d, 3, 5, Word(sta_absx)),
    /* 0x9e */ ins("???", 0x9e, 1, 2, Implied(unimplemented)),
    /* 0x9f */ ins("???", 0x9f, 1, 2, Implied(unimplemented)),

    /* 0xa0 */ ins("LDY", 0xa0, 2, 2, Byte(ldy_imm)),
    /* 0xa1 */ ins("LDA", 0xa1, 2, 6, Byte(lda_indx)),
    /* 0xa2 */ ins("LDX", 0xa2, 2, 2, Byte(ldx_imm)),
    /* 0xa3 */ ins("???", 0xa3, 1, 2, Implied(unimplemented)),
    /* 0xa4 */ ins("LDY", 0xa4, 2, 3, Byte(ldy_zpg)),
    /* 0xa5 */ ins("LDA", 0xa5, 2, 3, Byte(lda_zpg)),
    /* 0xa6 */ ins("LDX", 0xa6, 2, 3, Byte(ldx_zpg)),
    /* 0xa7 */ ins("???", 0xa7, 1, 2, Implied(unimplemented)),
    /* 0xa8 */ ins("TAY", 0xa8, 1, 2, Implied(tay)),
    /* 0xa9 */ ins("LDA", 0xa9, 2, 2, Byte(lda_imm)),
    /* 0xaa */ ins("TAX", 0xaa, 1, 2, Implied(tax)),
    /* 0xab */ ins("???", 0xab, 1, 2, Implied(unimplemented)),
    /* 0xac */ ins("LDY", 0xac, 3, 4, Word(ldy_abs)),
    /* 0xad */ ins("LDA", 0xad, 3, 4, Word(lda_abs)),
    /* 0xae */ ins("LDX", 0xae, 3, 4, Word(ldx_abs)),
    /* 0xaf */ ins("???", 0xaf, 1, 2, Implied(unimplemented)),
    /* 0xb0 */ ins("BCS", 0xb0, 2, 2, Byte(bcs)),
    /* 0xb1 */ ins("LDA", 0xb1, 2, 5, Byte(lda_indy)),
    /* 0xb2 */ ins("???", 0xb2, 1, 2, Implied(unimplemented)),
    /* 0xb3 */ ins("???", 0xb3, 1, 2, Implied(unimplemented)),
    /* 0xb4 */ ins("LDY", 0xb4, 2, 4, Byte(ldy_zpgx)),
    /* 0xb5 */ ins("LDA", 0xb5, 2, 4, Byte(lda_zpgx)),
    /* 0xb6 */ ins("LDX", 0xb6, 2, 4, Byte(ldx_zpgy)),
    /* 0xb7 */ ins("???", 0xb7, 1, 2, Implied(unimplemented)),
    /* 0xb8 */ ins("CLV", 0xb8, 1, 2, Implied(clv)),
    /* 0xb9 */ ins("LDA", 0xb9, 3, 4, Word(lda_absy)),
    /* 0xba */ ins("TSX", 0xba, 1, 2, Implied(tsx)),
    /* 0xbb */ ins("???", 0xbb, 1, 2, Implied(unimplemented)),
    /* 0xbc */ ins("LDY", 0xbc, 3, 4, Word(ldy_absx)),
    /* 0xbd */ ins("LDA", 0xbd, 3, 4, Word(lda_absx)),
    /* 0xbe */ ins("LDX", 0xbe, 3, 4, Word(ldx_absy)),
    /* 0xbf */ ins("???", 0xbf, 1, 2, Implied(unimplemented)),

    /* 0xc0 */ ins("CPY", 0xc0, 2, 2, Byte(cpy_imm)),
    /* 0xc1 */ ins("CMP", 0xc1, 2, 6, Byte(cmp_indx)),
    /* 0xc2 */ ins("???", 0xc2, 1, 2, Implied(unimplemented)),
    /* 0xc3 */ ins("???", 0xc3, 1, 2, Implied(unimplemented)),
    /* 0xc4 */ ins("CPY", 0xc4, 2, 3, Byte(cpy_zpg)),
    /* 0xc5 */ ins("CMP", 0xc5, 2, 3, Byte(cmp_zpg)),
    /* 0xc6 */ ins("DEC", 0xc6, 2, 5, Byte(dec_zpg)),
    /* 0xc7 */ ins("???", 0xc7, 1, 2, Implied(unimplemented)),
    /* 0xc8 */ ins("INY", 0xc8, 1, 2, Implied(iny)),
    /* 0xc9 */ ins("CMP", 0xc9, 2, 2, Byte(cmp_imm)),
    /* 0xca */ ins("DEX", 0xca, 1, 2, Implied(dex)),
    /* 0xcb */ ins("???", 0xcb, 1, 2, Implied(unimplemented)),
    /* 0xcc */ ins("CPY", 0xcc, 3, 4, Word(cpy_abs)),
    /* 0xcd */ ins("CMP", 0xcd, 3, 4, Word(cmp_abs)),
    /* 0xce */ ins("DEC", 0xce, 3, 3, Word(dec_abs)),
    /* 0xcf */ ins("???", 0xcf, 1, 2, Implied(unimplemented)),
    /* 0xd0 */ ins("BNE", 0xd0, 2, 2, Byte(bne)),
    /* 0xd1 */ ins("CMP", 0xd1, 2, 5, Byte(cmp_indy)),
    /* 0xd2 */ ins("???", 0xd2, 1, 2, Implied(unimplemented)),
    /* 0xd3 */ ins("???", 0xd3, 1, 2, Implied(unimplemented)),
    /* 0xd4 */ ins("???", 0xd4, 1, 2, Implied(unimplemented)),
    /* 0xd5 */ ins("CMP", 0xd5, 2, 4, Byte(cmp_zpgx)),
    /* 0xd6 */ ins("DEC", 0xd6, 2, 6, Byte(dec_zpgx)),
    /* 0xd7 */ ins("???", 0xd7, 1, 2, Implied(unimplemented)),
    /* 0xd8 */ ins("CLD", 0xd8, 1, 2, Implied(cld)),
    /* 0xd9 */ ins("CMP", 0xd9, 3, 4, Word(cmp_absy)),
    /* 0xda */ ins("???", 0xda, 1, 2, Implied(unimplemented)),
    /* 0xdb */ ins("???", 0xdb, 1, 2, Implied(unimplemented)),
    /* 0xdc */ ins("???", 0xdc, 1, 2, Implied(unimplemented)),
    /* 0xdd */ ins("CMP", 0xdd, 3, 4, Word(cmp_absx)),
    /* 0xde */ ins("DEC", 0xde, 3, 7, Word(dec_absx)),
    /* 0xdf */ ins("???", 0xdf, 1, 2, Implied(unimplemented)),

    /* 0xe0 */ ins("CPX", 0xe0, 2, 2, Byte(cpx_imm)),
    /* 0xe1 */ ins("SBC", 0xe1, 2, 2, Byte(sbc_indx)),
    /* 0xe2 */ ins("???", 0xe2, 1, 2, Implied(unimplemented)),
    /* 0xe3 */ ins("???", 0xe3, 1, 2, Implied(unimplemented)),
    /* 0xe4 */ ins("CPX", 0xe4, 2, 3, Byte(cpx_zpg)),
    /* 0xe5 */ ins("SBC", 0xe5, 2, 2, Byte(sbc_zpg)),
    /* 0xe6 */ ins("INC", 0xe6, 2, 5, Byte(inc_zpg)),
    /* 0xe7 */ ins("???", 0xe7, 1, 2, Implied(unimplemented)),
    /* 0xe8 */ ins("INX", 0xe8, 1, 2, Implied(inx)),
    /* 0xe9 */ ins("SBC", 0xe9, 2, 2, Byte(sbc_imm)),
    /* 0xea */ ins("NOP", 0xea, 1, 2, Implied(nop)),
    /* 0xeb */ ins("???", 0xeb, 1, 2, Implied(unimplemented)),
    /* 0xec */ ins("CPX", 0xec, 3, 4, Word(cpx_abs)),
    /* 0xed */ ins("SBC", 0xed, 3, 2, Word(sbc_abs)),
    /* 0xee */ ins("INC", 0xee, 3, 6, Word(inc_abs)),
    /* 0xef */ ins("???", 0xef, 1, 2, Implied(unimplemented)),
    /* 0xf0 */ ins("BEQ", 0xf0, 2, 2, Byte(beq)),
    /* 0xf1 */ ins("SBC", 0xf1, 2, 2, Byte(sbc_indy)),
    /* 0xf2 */ ins("???", 0xf2, 1, 2, Implied(unimplemented)),
    /* 0xf3 */ ins("???", 0xf3, 1, 2, Implied(unimplemented)),
    /* 0xf4 */ ins("???", 0xf4, 1, 2, Implied(unimplemented)),
    /* 0xf5 */ ins("SBC", 0xf5, 2, 2, Byte(sbc_zpgx)),
    /* 0xf6 */ ins("INC", 0xf6, 2, 6, Byte(inc_zpgx)),
    /* 0xf7 */ ins("???", 0xf7, 1, 2, Implied(unimplemented)),
    /* 0xf8 */ ins("SED", 0xf8, 1, 2, Implied(sed)),
    /* 0xf9 */ ins("SBC", 0xf9, 3, 2, Word(sbc_absy)),
    /* 0xfa */ ins("???", 0xfa, 1, 2, Implied(unimplemented)),
    /* 0xfb */ ins("???", 0xfb, 1, 2, Implied(unimplemented)),
    /* 0xfc */ ins("???", 0xfc, 1, 2, Implied(unimplemented)),
    /* 0xfd */ ins("SBC", 0xfd, 3, 2, Word(sbc_absx)),
    /* 0xfe */ ins("INC", 0xfe, 3, 7, Word(inc_absx)),
    /* 0xff */ ins("???", 0xff, 1, 2, Implied(unimplemented)),
];

// EWM_CPU_MODEL_65C02

fn ora_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_ind(bus, oper);
    ora(cpu, m);
}

fn and_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_ind(bus, oper);
    and(cpu, m);
}

fn eor_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_ind(bus, oper);
    eor(cpu, m);
}

fn adc_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_ind(bus, oper);
    adc(cpu, m);
}

fn sta_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_ind(bus, oper, cpu.a);
}

fn lda_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.a = mem_get_byte_ind(bus, oper);
    update_zn(cpu, cpu.a);
}

fn cmp_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_ind(bus, oper);
    cmp(cpu, m);
}

fn sbc_ind(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_ind(bus, oper);
    sbc(cpu, m);
}

fn bit_imm(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    // Immediate-mode BIT only affects the z flag, as in ins.c.
    let t = cpu.a & oper;
    cpu.z = (t == 0) as u8;
}

fn bit_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    let m = mem_get_byte_zpgx(cpu, bus, oper);
    bit(cpu, m);
}

fn bit_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    let m = mem_get_byte_absx(cpu, bus, oper);
    bit(cpu, m);
}

fn dec_acc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = cpu.a.wrapping_sub(1);
    update_zn(cpu, cpu.a);
}

fn inc_acc(cpu: &mut Cpu, _bus: &mut dyn Bus) {
    cpu.a = cpu.a.wrapping_add(1);
    update_zn(cpu, cpu.a);
}

fn jmp_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.pc = bus.read_word(oper.wrapping_add(cpu.x as u16));
}

fn bra(cpu: &mut Cpu, _bus: &mut dyn Bus, oper: u8) {
    cpu.pc = cpu.pc.wrapping_add((oper as i8) as u16);
}

fn phx(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.push_byte(bus, cpu.x);
}

fn phy(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.push_byte(bus, cpu.y);
}

fn plx(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.x = cpu.pull_byte(bus);
    update_zn(cpu, cpu.x);
}

fn ply(cpu: &mut Cpu, bus: &mut dyn Bus) {
    cpu.y = cpu.pull_byte(bus);
    update_zn(cpu, cpu.y);
}

fn stz_zpg(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpg(bus, oper, 0x00);
}

fn stz_zpgx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    mem_set_byte_zpgx(cpu, bus, oper, 0x00);
}

fn stz_abs(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_abs(bus, oper, 0x00);
}

fn stz_absx(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    mem_set_byte_absx(cpu, bus, oper, 0x00);
}

fn trb_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.z = ((bus.read(oper as u16) & cpu.a) == 0) as u8;
    let r = bus.read(oper as u16) & !cpu.a;
    mem_set_byte_zpg(bus, oper, r);
}

fn trb_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.z = ((bus.read(oper) & cpu.a) == 0) as u8;
    let r = bus.read(oper) & (cpu.a ^ 0xff);
    mem_set_byte_abs(bus, oper, r);
}

fn tsb_zpg(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    cpu.z = ((bus.read(oper as u16) & cpu.a) == 0) as u8;
    let r = bus.read(oper as u16) | cpu.a;
    mem_set_byte_zpg(bus, oper, r);
}

fn tsb_abs(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    cpu.z = ((bus.read(oper) & cpu.a) == 0) as u8;
    let r = bus.read(oper) | cpu.a;
    mem_set_byte_abs(bus, oper, r);
}

// BBR/BBS are 3-byte instructions dispatched as Word: the zero-page address
// rides in the operand low byte and the relative branch in the high byte,
// exactly as the C handlers unpack them.

fn bbr(cpu: &mut Cpu, bus: &mut dyn Bus, bit: u8, zp: u8, label: i8) {
    if (mem_get_byte_zpg(bus, zp) & bit) == 0 {
        cpu.pc = cpu.pc.wrapping_add(label as u16);
    }
}

fn bbr0(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0000_0001,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr1(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0000_0010,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr2(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0000_0100,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr3(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0000_1000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr4(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0001_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr5(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0010_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr6(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b0100_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbr7(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbr(
        cpu,
        bus,
        0b1000_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs(cpu: &mut Cpu, bus: &mut dyn Bus, bit: u8, zp: u8, label: i8) {
    if (mem_get_byte_zpg(bus, zp) & bit) != 0 {
        cpu.pc = cpu.pc.wrapping_add(label as u16);
    }
}

fn bbs0(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0000_0001,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs1(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0000_0010,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs2(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0000_0100,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs3(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0000_1000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs4(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0001_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs5(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0010_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs6(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b0100_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn bbs7(cpu: &mut Cpu, bus: &mut dyn Bus, oper: u16) {
    bbs(
        cpu,
        bus,
        0b1000_0000,
        (oper & 0x00ff) as u8,
        (oper >> 8) as i8,
    );
}

fn rmb(bus: &mut dyn Bus, bit: u8, zp: u8) {
    let v = bus.read(zp as u16) & !bit;
    mem_set_byte_zpg(bus, zp, v);
}

fn rmb0(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0000_0001, oper);
}

fn rmb1(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0000_0010, oper);
}

fn rmb2(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0000_0100, oper);
}

fn rmb3(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0000_1000, oper);
}

fn rmb4(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0001_0000, oper);
}

fn rmb5(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0010_0000, oper);
}

fn rmb6(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b0100_0000, oper);
}

fn rmb7(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    rmb(bus, 0b1000_0000, oper);
}

fn smb(bus: &mut dyn Bus, bit: u8, zp: u8) {
    let v = bus.read(zp as u16) | bit;
    mem_set_byte_zpg(bus, zp, v);
}

fn smb0(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0000_0001, oper);
}

fn smb1(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0000_0010, oper);
}

fn smb2(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0000_0100, oper);
}

fn smb3(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0000_1000, oper);
}

fn smb4(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0001_0000, oper);
}

fn smb5(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0010_0000, oper);
}

fn smb6(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b0100_0000, oper);
}

fn smb7(_cpu: &mut Cpu, bus: &mut dyn Bus, oper: u8) {
    smb(bus, 0b1000_0000, oper);
}

// The C table casts the 1-argument nop through 2- and 3-byte handler
// pointers; Rust needs arity-matched wrappers instead.

fn nop_byte(_cpu: &mut Cpu, _bus: &mut dyn Bus, _oper: u8) {}

fn nop_word(_cpu: &mut Cpu, _bus: &mut dyn Bus, _oper: u16) {}

/// The 65C02 override table, transcribed verbatim from `instructions_65C02`
/// in ins.c — including its oddities (several `NOP` entries with cycles=1,
/// and the `opcode` field reading 0x03 on every 0xX3 slot). `None` entries
/// (`NULL` handlers in C) are backfilled from the 6502 table, mirroring
/// `cpu_initialize()`.
#[rustfmt::skip]
static INSTRUCTIONS_65C02_OVERRIDES: [Option<Instruction>; 256] = [
    /* 0x00 */ None,
    /* 0x01 */ None,
    /* 0x02 */ Some(ins("NOP", 0x02, 2, 2, Byte(nop_byte))),
    /* 0x03 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x04 */ Some(ins("TSB", 0x04, 2, 5, Byte(tsb_zpg))),
    /* 0x05 */ None,
    /* 0x06 */ None,
    /* 0x07 */ Some(ins("RMB", 0x07, 2, 5, Byte(rmb0))),
    /* 0x08 */ None,
    /* 0x09 */ None,
    /* 0x0a */ None,
    /* 0x0b */ Some(ins("NOP", 0x0b, 1, 1, Implied(nop))),
    /* 0x0c */ Some(ins("TSB", 0x0c, 3, 6, Word(tsb_abs))),
    /* 0x0d */ None,
    /* 0x0e */ None,
    /* 0x0f */ Some(ins("BBR", 0x0f, 3, 5, Word(bbr0))),
    /* 0x10 */ None,
    /* 0x11 */ None,
    /* 0x12 */ Some(ins("ORA", 0x12, 2, 5, Byte(ora_ind))),
    /* 0x13 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x14 */ Some(ins("TRB", 0x14, 2, 5, Byte(trb_zpg))),
    /* 0x15 */ None,
    /* 0x16 */ None,
    /* 0x17 */ Some(ins("RMB", 0x17, 2, 5, Byte(rmb1))),
    /* 0x18 */ None,
    /* 0x19 */ None,
    /* 0x1a */ Some(ins("INC", 0x1a, 1, 2, Implied(inc_acc))),
    /* 0x1b */ Some(ins("NOP", 0x1b, 1, 1, Implied(nop))),
    /* 0x1c */ Some(ins("TRB", 0x1c, 3, 6, Word(trb_abs))),
    /* 0x1d */ None,
    /* 0x1e */ None,
    /* 0x1f */ Some(ins("BBR", 0x1f, 3, 5, Word(bbr1))),
    /* 0x20 */ None,
    /* 0x21 */ None,
    /* 0x22 */ Some(ins("NOP", 0x22, 2, 2, Byte(nop_byte))),
    /* 0x23 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x24 */ None,
    /* 0x25 */ None,
    /* 0x26 */ None,
    /* 0x27 */ Some(ins("RMB", 0x27, 2, 5, Byte(rmb2))),
    /* 0x28 */ None,
    /* 0x29 */ None,
    /* 0x2a */ None,
    /* 0x2b */ Some(ins("NOP", 0x2b, 1, 1, Implied(nop))),
    /* 0x2c */ None,
    /* 0x2d */ None,
    /* 0x2e */ None,
    /* 0x2f */ Some(ins("BBR", 0x2f, 3, 5, Word(bbr2))),
    /* 0x30 */ None,
    /* 0x31 */ None,
    /* 0x32 */ Some(ins("AND", 0x32, 2, 5, Byte(and_ind))),
    /* 0x33 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x34 */ Some(ins("BIT", 0x34, 2, 4, Byte(bit_zpgx))),
    /* 0x35 */ None,
    /* 0x36 */ None,
    /* 0x37 */ Some(ins("RMB", 0x37, 2, 5, Byte(rmb3))),
    /* 0x38 */ None,
    /* 0x39 */ None,
    /* 0x3a */ Some(ins("DEC", 0x3a, 1, 2, Implied(dec_acc))),
    /* 0x3b */ Some(ins("NOP", 0x3b, 1, 1, Implied(nop))),
    /* 0x3c */ Some(ins("BIT", 0x3c, 3, 4, Word(bit_absx))),
    /* 0x3d */ None,
    /* 0x3e */ None,
    /* 0x3f */ Some(ins("BBR", 0x3f, 3, 5, Word(bbr3))),
    /* 0x40 */ None,
    /* 0x41 */ None,
    /* 0x42 */ Some(ins("NOP", 0x42, 2, 2, Byte(nop_byte))),
    /* 0x43 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x44 */ Some(ins("NOP", 0x44, 2, 3, Byte(nop_byte))),
    /* 0x45 */ None,
    /* 0x46 */ None,
    /* 0x47 */ Some(ins("RMB", 0x47, 2, 5, Byte(rmb4))),
    /* 0x48 */ None,
    /* 0x49 */ None,
    /* 0x4a */ None,
    /* 0x4b */ Some(ins("NOP", 0x4b, 1, 1, Implied(nop))),
    /* 0x4c */ None,
    /* 0x4d */ None,
    /* 0x4e */ None,
    /* 0x4f */ Some(ins("BBR", 0x4f, 3, 5, Word(bbr4))),
    /* 0x50 */ None,
    /* 0x51 */ None,
    /* 0x52 */ Some(ins("EOR", 0x52, 2, 5, Byte(eor_ind))),
    /* 0x53 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x54 */ Some(ins("NOP", 0x54, 2, 4, Byte(nop_byte))),
    /* 0x55 */ None,
    /* 0x56 */ None,
    /* 0x57 */ Some(ins("RMB", 0x57, 2, 5, Byte(rmb5))),
    /* 0x58 */ None,
    /* 0x59 */ None,
    /* 0x5a */ Some(ins("PHY", 0x5a, 1, 3, Implied(phy))),
    /* 0x5b */ Some(ins("NOP", 0x5b, 1, 1, Implied(nop))),
    /* 0x5c */ Some(ins("NOP", 0x5c, 3, 8, Word(nop_word))),
    /* 0x5d */ None,
    /* 0x5e */ None,
    /* 0x5f */ Some(ins("BBR", 0x5f, 3, 5, Word(bbr5))),
    /* 0x60 */ None,
    /* 0x61 */ None,
    /* 0x62 */ Some(ins("NOP", 0x62, 2, 2, Byte(nop_byte))),
    /* 0x63 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x64 */ Some(ins("STZ", 0x64, 2, 3, Byte(stz_zpg))),
    /* 0x65 */ None,
    /* 0x66 */ None,
    /* 0x67 */ Some(ins("RMB", 0x67, 2, 5, Byte(rmb6))),
    /* 0x68 */ None,
    /* 0x69 */ None,
    /* 0x6a */ None,
    /* 0x6b */ Some(ins("NOP", 0x6b, 1, 1, Implied(nop))),
    /* 0x6c */ None,
    /* 0x6d */ None,
    /* 0x6e */ None,
    /* 0x6f */ Some(ins("BBR", 0x6f, 3, 5, Word(bbr6))),
    /* 0x70 */ None,
    /* 0x71 */ None,
    /* 0x72 */ Some(ins("ADC", 0x72, 2, 5, Byte(adc_ind))),
    /* 0x73 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x74 */ Some(ins("STZ", 0x74, 2, 4, Byte(stz_zpgx))),
    /* 0x75 */ None,
    /* 0x76 */ None,
    /* 0x77 */ Some(ins("RMB", 0x77, 2, 5, Byte(rmb7))),
    /* 0x78 */ None,
    /* 0x79 */ None,
    /* 0x7a */ Some(ins("PLY", 0x7a, 1, 4, Implied(ply))),
    /* 0x7b */ Some(ins("NOP", 0x7b, 1, 1, Implied(nop))),
    /* 0x7c */ Some(ins("JMP", 0x7c, 3, 6, Word(jmp_absx))),
    /* 0x7d */ None,
    /* 0x7e */ None,
    /* 0x7f */ Some(ins("BBR", 0x7f, 3, 5, Word(bbr7))),
    /* 0x80 */ Some(ins("BRA", 0x80, 2, 3, Byte(bra))),
    /* 0x81 */ None,
    /* 0x82 */ Some(ins("NOP", 0x82, 2, 2, Byte(nop_byte))),
    /* 0x83 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x84 */ None,
    /* 0x85 */ None,
    /* 0x86 */ None,
    /* 0x87 */ Some(ins("SMB", 0x87, 2, 5, Byte(smb0))),
    /* 0x88 */ None,
    /* 0x89 */ Some(ins("BIT", 0x89, 2, 2, Byte(bit_imm))),
    /* 0x8a */ None,
    /* 0x8b */ Some(ins("NOP", 0x8b, 1, 1, Implied(nop))),
    /* 0x8c */ None,
    /* 0x8d */ None,
    /* 0x8e */ None,
    /* 0x8f */ Some(ins("BBS", 0x8f, 3, 5, Word(bbs0))),
    /* 0x90 */ None,
    /* 0x91 */ None,
    /* 0x92 */ Some(ins("STA", 0x92, 2, 5, Byte(sta_ind))),
    /* 0x93 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0x94 */ None,
    /* 0x95 */ None,
    /* 0x96 */ None,
    /* 0x97 */ Some(ins("SMB", 0x97, 2, 5, Byte(smb1))),
    /* 0x98 */ None,
    /* 0x99 */ None,
    /* 0x9a */ None,
    /* 0x9b */ Some(ins("NOP", 0x9b, 1, 1, Implied(nop))),
    /* 0x9c */ Some(ins("STZ", 0x9c, 3, 4, Word(stz_abs))),
    /* 0x9d */ None,
    /* 0x9e */ Some(ins("STZ", 0x9e, 3, 5, Word(stz_absx))),
    /* 0x9f */ Some(ins("BBS", 0x9f, 3, 5, Word(bbs1))),
    /* 0xa0 */ None,
    /* 0xa1 */ None,
    /* 0xa2 */ None,
    /* 0xa3 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0xa4 */ None,
    /* 0xa5 */ None,
    /* 0xa6 */ None,
    /* 0xa7 */ Some(ins("SMB", 0xa7, 2, 5, Byte(smb2))),
    /* 0xa8 */ None,
    /* 0xa9 */ None,
    /* 0xaa */ None,
    /* 0xab */ Some(ins("NOP", 0xab, 1, 1, Implied(nop))),
    /* 0xac */ None,
    /* 0xad */ None,
    /* 0xae */ None,
    /* 0xaf */ Some(ins("BBS", 0xaf, 3, 5, Word(bbs2))),
    /* 0xb0 */ None,
    /* 0xb1 */ None,
    /* 0xb2 */ Some(ins("LDA", 0xb2, 2, 5, Byte(lda_ind))),
    /* 0xb3 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0xb4 */ None,
    /* 0xb5 */ None,
    /* 0xb6 */ None,
    /* 0xb7 */ Some(ins("SMB", 0xb7, 2, 5, Byte(smb3))),
    /* 0xb8 */ None,
    /* 0xb9 */ None,
    /* 0xba */ None,
    /* 0xbb */ Some(ins("NOP", 0xbb, 1, 1, Implied(nop))),
    /* 0xbc */ None,
    /* 0xbd */ None,
    /* 0xbe */ None,
    /* 0xbf */ Some(ins("BBS", 0xbf, 3, 5, Word(bbs3))),
    /* 0xc0 */ None,
    /* 0xc1 */ None,
    /* 0xc2 */ Some(ins("NOP", 0xc2, 2, 2, Byte(nop_byte))),
    /* 0xc3 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0xc4 */ None,
    /* 0xc5 */ None,
    /* 0xc6 */ None,
    /* 0xc7 */ Some(ins("SMB", 0xc7, 2, 5, Byte(smb4))),
    /* 0xc8 */ None,
    /* 0xc9 */ None,
    /* 0xca */ None,
    /* 0xcb */ Some(ins("NOP", 0xcb, 1, 1, Implied(nop))),
    /* 0xcc */ None,
    /* 0xcd */ None,
    /* 0xce */ None,
    /* 0xcf */ Some(ins("BBS", 0xcf, 3, 5, Word(bbs4))),
    /* 0xd0 */ None,
    /* 0xd1 */ None,
    /* 0xd2 */ Some(ins("CMP", 0xd2, 2, 5, Byte(cmp_ind))),
    /* 0xd3 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0xd4 */ Some(ins("NOP", 0xd4, 2, 4, Byte(nop_byte))),
    /* 0xd5 */ None,
    /* 0xd6 */ None,
    /* 0xd7 */ Some(ins("SMB", 0xd7, 2, 5, Byte(smb5))),
    /* 0xd8 */ None,
    /* 0xd9 */ None,
    /* 0xda */ Some(ins("PHX", 0xda, 1, 3, Implied(phx))),
    /* 0xdb */ Some(ins("NOP", 0xdb, 1, 1, Implied(nop))),
    /* 0xdc */ Some(ins("NOP", 0xdc, 3, 4, Word(nop_word))),
    /* 0xdd */ None,
    /* 0xde */ None,
    /* 0xdf */ Some(ins("BBS", 0xdf, 3, 5, Word(bbs5))),
    /* 0xe0 */ None,
    /* 0xe1 */ None,
    /* 0xe2 */ Some(ins("NOP", 0xe2, 2, 2, Byte(nop_byte))),
    /* 0xe3 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0xe4 */ None,
    /* 0xe5 */ None,
    /* 0xe6 */ None,
    /* 0xe7 */ Some(ins("SMB", 0xe7, 2, 5, Byte(smb6))),
    /* 0xe8 */ None,
    /* 0xe9 */ None,
    /* 0xea */ None,
    /* 0xeb */ Some(ins("NOP", 0xeb, 1, 1, Implied(nop))),
    /* 0xec */ None,
    /* 0xed */ None,
    /* 0xee */ None,
    /* 0xef */ Some(ins("BBS", 0xef, 3, 5, Word(bbs6))),
    /* 0xf0 */ None,
    /* 0xf1 */ None,
    /* 0xf2 */ Some(ins("SBC", 0xf2, 2, 5, Byte(sbc_ind))),
    /* 0xf3 */ Some(ins("NOP", 0x03, 1, 1, Implied(nop))),
    /* 0xf4 */ Some(ins("NOP", 0xf4, 2, 4, Byte(nop_byte))),
    /* 0xf5 */ None,
    /* 0xf6 */ None,
    /* 0xf7 */ Some(ins("SMB", 0xf7, 2, 5, Byte(smb7))),
    /* 0xf8 */ None,
    /* 0xf9 */ None,
    /* 0xfa */ Some(ins("PLX", 0xfa, 1, 4, Implied(plx))),
    /* 0xfb */ Some(ins("NOP", 0xfb, 1, 1, Implied(nop))),
    /* 0xfc */ Some(ins("NOP", 0xfc, 3, 4, Word(nop_word))),
    /* 0xfd */ None,
    /* 0xfe */ None,
    /* 0xff */ Some(ins("BBS", 0xff, 3, 5, Word(bbs7))),
];

/// The 65C02 table: the 6502 table with the overrides overlaid, built once —
/// the Rust version of `cpu_initialize()`'s back-fill in cpu.c.
pub fn instructions_65c02() -> &'static [Instruction; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[Instruction; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = INSTRUCTIONS_6502;
        for (slot, or) in table.iter_mut().zip(INSTRUCTIONS_65C02_OVERRIDES.iter()) {
            if let Some(instruction) = or {
                *slot = *instruction;
            }
        }
        table
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::TestBus;

    #[test]
    fn table_is_positionally_consistent() {
        for (i, ins) in INSTRUCTIONS_6502.iter().enumerate() {
            assert_eq!(ins.opcode as usize, i, "opcode mismatch at slot {i:#04x}");
            // The bytes field must match the handler arity the dispatcher
            // pairs it with.
            let expected = match ins.handler {
                Handler::Implied(_) => 1,
                Handler::Byte(_) => 2,
                Handler::Word(_) => 3,
            };
            assert_eq!(ins.bytes, expected, "bytes/arity mismatch at {i:#04x}");
        }
    }

    #[test]
    fn indx_wraps_in_zero_page() {
        let mut cpu = Cpu::new(Model::M6502);
        let mut bus = TestBus::new();
        cpu.x = 1;
        // Pointer for ($fe,X) with X=1: low byte at $ff, high byte wraps to $00.
        bus.write(0x00ff, 0x34);
        bus.write(0x0000, 0x12);
        bus.write(0x1234, 0x42);
        assert_eq!(mem_get_byte_indx(&cpu, &mut bus, 0xfe), 0x42);
    }

    #[test]
    fn indy_does_not_wrap_in_zero_page() {
        // ($ff),Y reads its pointer high byte from $0100, not $0000 — the C
        // code's int promotion in mem_get_byte_indy, preserved on purpose.
        let mut cpu = Cpu::new(Model::M6502);
        let mut bus = TestBus::new();
        cpu.y = 2;
        bus.write(0x00ff, 0x00); // pointer low
        bus.write(0x0100, 0x20); // pointer high (page 1, not zero page!)
        bus.write(0x0000, 0x99); // decoy: would be the high byte if it wrapped
        bus.write(0x2002, 0x42);
        assert_eq!(mem_get_byte_indy(&cpu, &mut bus, 0xff), 0x42);
    }
}
