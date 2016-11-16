// The MIT License (MIT)
//
// Copyright (c) 2015 Stefan Arentz - http://github.com/st3fan/ewm
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#include <stdio.h>
#include <stdlib.h>

#include "ins.h"
#include "cpu.h"
#include "mem.h"

static void update_zn(struct cpu_t *cpu, uint8_t v) {
  cpu->state.z = (v == 0x00);
  cpu->state.n = (v & 0x80);
}

/* ADC */

static void adc(struct cpu_t *cpu, uint8_t m) {
  uint16_t t = (uint16_t)cpu->state.a + (uint16_t)m + (uint16_t)(cpu->state.c ? 1 : 0);
  uint8_t r = (uint8_t) (t & 0xff);
  cpu->state.c = (t & 0x0100) != 0;
  cpu->state.v = (cpu->state.a^r) & (m^r) & 0x80;
  cpu->state.a = r;
  update_zn(cpu, cpu->state.a);
}

static void adc_imm(struct cpu_t *cpu, uint8_t oper) {
  adc(cpu, oper);
}

static void adc_zpg(struct cpu_t *cpu, uint8_t oper) {
  adc(cpu, mem_get_byte_zpg(cpu, oper));
}

static void adc_zpgx(struct cpu_t *cpu, uint8_t oper) {
  adc(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void adc_abs(struct cpu_t *cpu, uint16_t oper) {
  adc(cpu, mem_get_byte_abs(cpu, oper));
}

static void adc_absx(struct cpu_t *cpu, uint16_t oper) {
  adc(cpu, mem_get_byte_absx(cpu, oper));
}

static void adc_absy(struct cpu_t *cpu, uint16_t oper) {
  adc(cpu, mem_get_byte_absy(cpu, oper));
}

static void adc_indx(struct cpu_t *cpu, uint8_t oper) {
  adc(cpu, mem_get_byte_indx(cpu, oper));
}

static void adc_indy(struct cpu_t *cpu, uint8_t oper) {
  adc(cpu, mem_get_byte_indy(cpu, oper));
}

/* AND */

static void and(struct cpu_t *cpu, uint8_t m) {
  cpu->state.a &= m;
  update_zn(cpu, cpu->state.a);
}

static void and_imm(struct cpu_t *cpu, uint8_t oper) {
  and(cpu, oper);
}

static void and_zpg(struct cpu_t *cpu, uint8_t oper) {
  and(cpu, mem_get_byte_zpg(cpu, oper));
}

static void and_zpgx(struct cpu_t *cpu, uint8_t oper) {
  and(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void and_abs(struct cpu_t *cpu, uint16_t oper) {
  and(cpu, mem_get_byte_abs(cpu, oper));
}

static void and_absx(struct cpu_t *cpu, uint16_t oper) {
  and(cpu, mem_get_byte_absx(cpu, oper));
}

static void and_absy(struct cpu_t *cpu, uint16_t oper) {
  and(cpu, mem_get_byte_absy(cpu, oper));
}

static void and_indx(struct cpu_t *cpu, uint8_t oper) {
  and(cpu, mem_get_byte_indx(cpu, oper));
}

static void and_indy(struct cpu_t *cpu, uint8_t oper) {
  and(cpu, mem_get_byte_indy(cpu, oper));
}

/* ASL */

static uint8_t asl(struct cpu_t *cpu, uint8_t b) {
  cpu->state.c = (b & 0x80);
  b <<= 1;
  cpu->state.n = (b & 0x80);
  cpu->state.z = (b == 0);
  return b;
}

static void asl_acc(struct cpu_t *cpu) {
  cpu->state.a = asl(cpu, cpu->state.a);
}

static void asl_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpg(cpu, oper, asl);
}

static void asl_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpgx(cpu, oper, asl);
}

static void asl_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_abs(cpu, oper, asl);
}

static void asl_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_absx(cpu, oper, asl);
}

/* BIT */

static void bit(struct cpu_t *cpu, uint8_t m) {
  uint8_t t = cpu->state.a & m;
  cpu->state.n = (m & 0x80);
  cpu->state.v = (m & 0x40);
  cpu->state.z = (t == 0);
}

static void bit_zpg(struct cpu_t *cpu, uint8_t oper) {
  bit(cpu, mem_get_byte_zpg(cpu, oper));
}

static void bit_abs(struct cpu_t *cpu, uint16_t oper) {
  bit(cpu, mem_get_byte_abs(cpu, oper));
}

/* Bxx Branches */

static void bcc(struct cpu_t *cpu, uint8_t oper) {
  if (cpu->state.c == 0) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void bcs(struct cpu_t *cpu, uint8_t oper) {
  if (cpu->state.c) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void beq(struct cpu_t *cpu, uint8_t oper) {
  if (cpu->state.z) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void bmi(struct cpu_t *cpu, uint8_t oper) {
  if (cpu->state.n) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void bne(struct cpu_t *cpu, uint8_t oper) {
  if (!cpu->state.z) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void bpl(struct cpu_t *cpu, uint8_t oper) {
  if (!cpu->state.n) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void bvc(struct cpu_t *cpu, uint8_t oper) {
  if (!cpu->state.v) {
    cpu->state.pc += (int8_t) oper;
  }
}

static void bvs(struct cpu_t *cpu, uint8_t oper) {
  if (cpu->state.v) {
    cpu->state.pc += (int8_t) oper;
  }
}

/* BRK */

static void brk(struct cpu_t *cpu) {
  cpu->state.b = 1;
  cpu_irq(cpu);
}

/* CLx */

static void clc(struct cpu_t *cpu) {
  cpu->state.c = 0;
}

static void cld(struct cpu_t *cpu) {
  cpu->state.d = 0;
}

static void cli(struct cpu_t *cpu) {
  cpu->state.i = 0;
}

static void clv(struct cpu_t *cpu) {
  cpu->state.v = 0;
}

/* CMP */

static void cmp(struct cpu_t *cpu, uint8_t m) {
  uint8_t t = cpu->state.a - m;
  cpu->state.c = (cpu->state.a >= m);
  cpu->state.n = (t & 0x80);
  cpu->state.z = (t == 0);
}

static void cmp_imm(struct cpu_t *cpu, uint8_t oper) {
  cmp(cpu, oper);
}

static void cmp_zpg(struct cpu_t *cpu, uint8_t oper) {
  cmp(cpu, mem_get_byte_zpg(cpu, oper));
}

static void cmp_zpgx(struct cpu_t *cpu, uint8_t oper) {
  cmp(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void cmp_abs(struct cpu_t *cpu, uint16_t oper) {
  cmp(cpu, mem_get_byte_abs(cpu, oper));
}

static void cmp_absx(struct cpu_t *cpu, uint16_t oper) {
  cmp(cpu, mem_get_byte_absx(cpu, oper));
}

static void cmp_absy(struct cpu_t *cpu, uint16_t oper) {
  cmp(cpu, mem_get_byte_absy(cpu, oper));
}

static void cmp_indx(struct cpu_t *cpu, uint8_t oper) {
  cmp(cpu, mem_get_byte_indx(cpu, oper));
}

static void cmp_indy(struct cpu_t *cpu, uint8_t oper) {
  cmp(cpu, mem_get_byte_indy(cpu, oper));
}

/* CPX */

static void cpx(struct cpu_t *cpu, uint8_t m) {
  uint8_t t = cpu->state.x - m;
  cpu->state.c = (cpu->state.x >= m);
  update_zn(cpu, t);
}

static void cpx_imm(struct cpu_t *cpu, uint8_t oper) {
  cpx(cpu, oper);
}

static void cpx_zpg(struct cpu_t *cpu, uint8_t oper) {
  cpx(cpu, mem_get_byte_zpg(cpu, oper));
}

static void cpx_abs(struct cpu_t *cpu, uint16_t oper) {
  cpx(cpu, mem_get_byte_abs(cpu, oper));
}

/* CPY */

static void cpy(struct cpu_t *cpu, uint8_t m) {
  uint8_t t = cpu->state.y - m;
  cpu->state.c = (cpu->state.y >= m);
  update_zn(cpu, t);
}

static void cpy_imm(struct cpu_t *cpu, uint8_t oper) {
  cpy(cpu, oper);
}

static void cpy_zpg(struct cpu_t *cpu, uint8_t oper) {
  cpy(cpu, mem_get_byte_zpg(cpu, oper));
}

static void cpy_abs(struct cpu_t *cpu, uint16_t oper) {
  cpy(cpu, mem_get_byte_abs(cpu, oper));
}

/* DEx */

static uint8_t dec(struct cpu_t *cpu, uint8_t b) {
  uint8_t t = b - 1;
  update_zn(cpu, t);
  return t;
}

static void dec_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpg(cpu, oper, dec);
}

static void dec_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpgx(cpu, oper, dec);
}

static void dec_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_abs(cpu, oper, dec);
}

static void dec_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_absx(cpu, oper, dec);
}

static void dex(struct cpu_t *cpu) {
  cpu->state.x--;
  update_zn(cpu, cpu->state.x);
}

static void dey(struct cpu_t *cpu) {
  cpu->state.y--;
  update_zn(cpu, cpu->state.y);
}

/* EOR */

static void eor(struct cpu_t *cpu, uint8_t m) {
  cpu->state.a ^= m;
  update_zn(cpu, cpu->state.a);
}

static void eor_imm(struct cpu_t *cpu, uint8_t oper) {
  eor(cpu, oper);
}

static void eor_zpg(struct cpu_t *cpu, uint8_t oper) {
  eor(cpu, mem_get_byte_zpg(cpu, oper));
}

static void eor_zpgx(struct cpu_t *cpu, uint8_t oper) {
  eor(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void eor_abs(struct cpu_t *cpu, uint16_t oper) {
  eor(cpu, mem_get_byte_abs(cpu, oper));
}

static void eor_absx(struct cpu_t *cpu, uint16_t oper) {
  eor(cpu, mem_get_byte_absx(cpu, oper));
}

static void eor_absy(struct cpu_t *cpu, uint16_t oper) {
  eor(cpu, mem_get_byte_absy(cpu, oper));
}

static void eor_indx(struct cpu_t *cpu, uint8_t oper) {
  eor(cpu, mem_get_byte_indx(cpu, oper));
}

static void eor_indy(struct cpu_t *cpu, uint8_t oper) {
  eor(cpu, mem_get_byte_indy(cpu, oper));
}

/* INx */

static uint8_t inc(struct cpu_t *cpu, uint8_t b) {
  uint8_t t = b + 1;
  update_zn(cpu, t);
  return t;
}

static void inc_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpg(cpu, oper, inc);
}

static void inc_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpgx(cpu, oper, inc);
}

static void inc_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_abs(cpu, oper, inc);
}

static void inc_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_absx(cpu, oper, inc);
}

static void inx(struct cpu_t *cpu) {
  cpu->state.x++;
  update_zn(cpu, cpu->state.x);
}

static void iny(struct cpu_t *cpu) {
  cpu->state.y++;
  update_zn(cpu, cpu->state.y);
}

/* JMP */

static void jmp_abs(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.pc = oper;
}

static void jmp_ind(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.pc = mem_get_word(cpu, oper);
}

/* JSR */

static void jsr_abs(struct cpu_t *cpu, uint16_t oper) {
  _cpu_push_word(cpu, cpu->state.pc - 1);
  cpu->state.pc = oper;
}

/* LDA */

static void lda_imm(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.a = oper;
  update_zn(cpu, cpu->state.a);
}

static void lda_zpg(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.a = mem_get_byte_zpg(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

static void lda_zpgx(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.a = mem_get_byte_zpgx(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

static void lda_abs(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.a = mem_get_byte_abs(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

static void lda_absx(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.a = mem_get_byte_absx(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

static void lda_absy(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.a = mem_get_byte_absy(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

static void lda_indx(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.a = mem_get_byte_indx(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

static void lda_indy(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.a = mem_get_byte_indy(cpu, oper);
  update_zn(cpu, cpu->state.a);
}

/* LDX */

static void ldx_imm(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.x = oper;
  update_zn(cpu, cpu->state.x);
}

static void ldx_zpg(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.x = mem_get_byte_zpg(cpu, oper);
  update_zn(cpu, cpu->state.x);
}

static void ldx_zpgy(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.x = mem_get_byte_zpgy(cpu, oper);
  update_zn(cpu, cpu->state.x);
}

static void ldx_abs(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.x = mem_get_byte_abs(cpu, oper);
  update_zn(cpu, cpu->state.x);
}

static void ldx_absy(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.x = mem_get_byte_absy(cpu, oper);
  update_zn(cpu, cpu->state.x);
}

/* LDY */

static void ldy_imm(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.y = oper;
  update_zn(cpu, cpu->state.y);
}

static void ldy_zpg(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.y = mem_get_byte_zpg(cpu, oper);
  update_zn(cpu, cpu->state.y);
}

static void ldy_zpgx(struct cpu_t *cpu, uint8_t oper) {
  cpu->state.y = mem_get_byte_zpgx(cpu, oper);
  update_zn(cpu, cpu->state.y);
}

static void ldy_abs(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.y = mem_get_byte_abs(cpu, oper);
  update_zn(cpu, cpu->state.y);
}

static void ldy_absx(struct cpu_t *cpu, uint16_t oper) {
  cpu->state.y = mem_get_byte_absx(cpu, oper);
  update_zn(cpu, cpu->state.y);
}

/* LSR */

static uint8_t lsr(struct cpu_t *cpu, uint8_t b) {
  cpu->state.c = (b & 1);
  b >>= 1;
  update_zn(cpu, b);
  return b;
}

static void lsr_acc(struct cpu_t *cpu) {
  cpu->state.a = lsr(cpu, cpu->state.a);
}

static void lsr_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpg(cpu, oper, lsr);
}

static void lsr_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpgx(cpu, oper, lsr);
}

static void lsr_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_abs(cpu, oper, lsr);
}
static void lsr_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_absx(cpu, oper, lsr);
}

/* NOP */

static void nop(struct cpu_t *cpu) {
}

/* ORA */

static void ora(struct cpu_t *cpu, uint8_t m) {
  cpu->state.a |= m;
  update_zn(cpu, cpu->state.a);
}

static void ora_imm(struct cpu_t *cpu, uint8_t oper) {
  ora(cpu, oper);
}

static void ora_zpg(struct cpu_t *cpu, uint8_t oper) {
  ora(cpu, mem_get_byte_zpg(cpu, oper));
}

static void ora_zpgx(struct cpu_t *cpu, uint8_t oper) {
  ora(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void ora_abs(struct cpu_t *cpu, uint16_t oper) {
  ora(cpu, mem_get_byte_abs(cpu, oper));
}

static void ora_absx(struct cpu_t *cpu, uint16_t oper) {
  ora(cpu, mem_get_byte_absx(cpu, oper));
}

static void ora_absy(struct cpu_t *cpu, uint16_t oper) {
  ora(cpu, mem_get_byte_absy(cpu, oper));
}

static void ora_indx(struct cpu_t *cpu, uint8_t oper) {
  ora(cpu, mem_get_byte_indx(cpu, oper));
}

static void ora_indy(struct cpu_t *cpu, uint8_t oper) {
  ora(cpu, mem_get_byte_indy(cpu, oper));
}


/* P** */

static void pha(struct cpu_t *cpu) {
  _cpu_push_byte(cpu, cpu->state.a);
}

static void pla(struct cpu_t *cpu) {
  cpu->state.a = _cpu_pull_byte(cpu);
  update_zn(cpu, cpu->state.a);
}

/* ROL */

static uint8_t rol(struct cpu_t* cpu, uint8_t b) {
  uint8_t carry = cpu->state.c ? 1 : 0;
  cpu->state.c = (b & 0x80);
  b = (b << 1) | carry;
  update_zn(cpu, b);
  return b;
}

static void rol_acc(struct cpu_t *cpu) {
  cpu->state.a = rol(cpu, cpu->state.a);
}

static void rol_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpg(cpu, oper, rol);
}

static void rol_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpgx(cpu, oper, rol);
}

static void rol_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_abs(cpu, oper, rol);
}

static void rol_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_absx(cpu, oper, rol);
}

/* ROR */

static uint8_t ror(struct cpu_t* cpu, uint8_t b) {
  uint8_t carry = cpu->state.c ? 1 : 0;
  cpu->state.c = (b & 0x01);
  b = (b >> 1) | (carry << 7);
  update_zn(cpu, b);
  return b;
}

static void ror_acc(struct cpu_t *cpu) {
  cpu->state.a = ror(cpu, cpu->state.a);
}

static void ror_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpg(cpu, oper, ror);
}

static void ror_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_mod_byte_zpgx(cpu, oper, ror);
}

static void ror_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_abs(cpu, oper, ror);
}

static void ror_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_mod_byte_absx(cpu, oper, ror);
}

/* RTI */

static void rti(struct cpu_t *cpu) {
  _cpu_set_status(cpu, _cpu_pull_byte(cpu));
  cpu->state.pc = _cpu_pull_word(cpu);
}

/* RTS */

static void rts(struct cpu_t *cpu) {
  cpu->state.pc = _cpu_pull_word(cpu) + 1;
}

/* SBC */

static void sbc(struct cpu_t *cpu, uint8_t m) {
  adc(cpu, m ^ 0xff);
}

static void sbc_imm(struct cpu_t *cpu, uint8_t oper) {
  sbc(cpu, oper);
}

static void sbc_zpg(struct cpu_t *cpu, uint8_t oper) {
  sbc(cpu, mem_get_byte_zpg(cpu, oper));
}

static void sbc_zpgx(struct cpu_t *cpu, uint8_t oper) {
  sbc(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void sbc_abs(struct cpu_t *cpu, uint16_t oper) {
  sbc(cpu, mem_get_byte_abs(cpu, oper));
}

static void sbc_absx(struct cpu_t *cpu, uint16_t oper) {
  sbc(cpu, mem_get_byte_absx(cpu, oper));
}

static void sbc_absy(struct cpu_t *cpu, uint16_t oper) {
  sbc(cpu, mem_get_byte_absy(cpu, oper));
}

static void sbc_indx(struct cpu_t *cpu, uint8_t oper) {
  sbc(cpu, mem_get_byte_indx(cpu, oper));
}

static void sbc_indy(struct cpu_t *cpu, uint8_t oper) {
  sbc(cpu, mem_get_byte_indy(cpu, oper));
}

/* SEx */

static void sec(struct cpu_t *cpu) {
  cpu->state.c = 1;
}

static void sed(struct cpu_t *cpu) {
  cpu->state.d = 1;
}

static void sei(struct cpu_t *cpu) {
  cpu->state.i = 1;
}

/* STA */

static void sta_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_zpg(cpu, oper, cpu->state.a);
}

static void sta_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_zpgx(cpu, oper, cpu->state.a);
}

static void sta_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_set_byte_abs(cpu, oper, cpu->state.a);
}

static void sta_absx(struct cpu_t *cpu, uint16_t oper) {
  mem_set_byte_absx(cpu, oper, cpu->state.a);
}

static void sta_absy(struct cpu_t *cpu, uint16_t oper) {
  mem_set_byte_absy(cpu, oper, cpu->state.a);
}

static void sta_indx(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_indx(cpu, oper, cpu->state.a);
}

static void sta_indy(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_indy(cpu, oper, cpu->state.a);
}

/* STX */

static void stx_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_zpg(cpu, oper, cpu->state.x);
}

static void stx_zpgy(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_zpgy(cpu, oper, cpu->state.x);
}

static void stx_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_set_byte_abs(cpu, oper, cpu->state.x);
}

/* STY */

static void sty_zpg(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_zpg(cpu, oper, cpu->state.y);
}

static void sty_zpgx(struct cpu_t *cpu, uint8_t oper) {
  mem_set_byte_zpgx(cpu, oper, cpu->state.y);
}

static void sty_abs(struct cpu_t *cpu, uint16_t oper) {
  mem_set_byte_abs(cpu, oper, cpu->state.y);
}

/* Txx */

static void tax(struct cpu_t *cpu) {
  cpu->state.x = cpu->state.a;
  update_zn(cpu, cpu->state.x);
}

static void tay(struct cpu_t *cpu) {
  cpu->state.y = cpu->state.a;
  update_zn(cpu, cpu->state.y);
}

static void tsx(struct cpu_t *cpu) {
  cpu->state.x = cpu->state.sp;
  update_zn(cpu, cpu->state.x);
}

static void txa(struct cpu_t *cpu) {
  cpu->state.a = cpu->state.x;
  update_zn(cpu, cpu->state.a);
}

static void txs(struct cpu_t *cpu) {
  cpu->state.sp = cpu->state.x;
}

static void tya(struct cpu_t *cpu) {
  cpu->state.a = cpu->state.y;
  update_zn(cpu, cpu->state.a);
}

/* Instruction dispatch table */

cpu_instruction_t instructions[256] = {
  /* 0x00 */ { "BRK", 0x00, 1, 2, (void*) brk },
  /* 0x01 */ { "ORA", 0x01, 2, 6, (void*) ora_indx },
  /* 0x02 */ { "???", 0x02, 1, 2, (void*) NULL },
  /* 0x03 */ { "???", 0x03, 1, 2, (void*) NULL },
  /* 0x04 */ { "???", 0x04, 1, 2, (void*) NULL },
  /* 0x05 */ { "ORA", 0x05, 2, 2, (void*) ora_zpg },
  /* 0x06 */ { "ASL", 0x06, 2, 5, (void*) asl_zpg },
  /* 0x07 */ { "???", 0x07, 1, 2, (void*) NULL },
  /* 0x08 */ { "???", 0x08, 1, 2, (void*) NULL },
  /* 0x09 */ { "ORA", 0x09, 2, 2, (void*) ora_imm },
  /* 0x0a */ { "ASL", 0x0a, 1, 2, (void*) asl_acc },
  /* 0x0b */ { "???", 0x0b, 1, 2, (void*) NULL },
  /* 0x0c */ { "???", 0x0c, 1, 2, (void*) NULL },
  /* 0x0d */ { "ORA", 0x0d, 3, 4, (void*) ora_abs },
  /* 0x0e */ { "ASL", 0x0e, 3, 6, (void*) asl_abs },
  /* 0x0f */ { "???", 0x0f, 1, 2, (void*) NULL },
  /* 0x10 */ { "BPL", 0x10, 2, 2, (void*) bpl },
  /* 0x11 */ { "ORA", 0x11, 2, 5, (void*) ora_indy },
  /* 0x12 */ { "???", 0x12, 1, 2, (void*) NULL },
  /* 0x13 */ { "???", 0x13, 1, 2, (void*) NULL },
  /* 0x14 */ { "???", 0x14, 1, 2, (void*) NULL },
  /* 0x15 */ { "ORA", 0x15, 2, 3, (void*) ora_zpgx },
  /* 0x16 */ { "ASL", 0x16, 2, 6, (void*) asl_zpgx },
  /* 0x17 */ { "???", 0x17, 1, 2, (void*) NULL },
  /* 0x18 */ { "CLC", 0x18, 1, 2, (void*) clc },
  /* 0x19 */ { "ORA", 0x19, 3, 4, (void*) ora_absy },
  /* 0x1a */ { "???", 0x1a, 1, 2, (void*) NULL },
  /* 0x1b */ { "???", 0x1b, 1, 2, (void*) NULL },
  /* 0x1c */ { "???", 0x1c, 1, 2, (void*) NULL },
  /* 0x1d */ { "ORA", 0x1d, 3, 4, (void*) ora_absx },
  /* 0x1e */ { "ASL", 0x1e, 3, 7, (void*) asl_absx },
  /* 0x1f */ { "???", 0x1f, 1, 2, (void*) NULL },

  /* 0x20 */ { "JSR", 0x20, 3, 6, (void*) jsr_abs },
  /* 0x21 */ { "AND", 0x21, 2, 6, (void*) and_indx },
  /* 0x22 */ { "???", 0x22, 1, 2, (void*) NULL },
  /* 0x23 */ { "???", 0x23, 1, 2, (void*) NULL },
  /* 0x24 */ { "BIT", 0x24, 2, 3, (void*) bit_zpg },
  /* 0x25 */ { "AND", 0x25, 2, 3, (void*) and_zpg },
  /* 0x26 */ { "ROL", 0x26, 2, 5, (void*) rol_zpg },
  /* 0x27 */ { "???", 0x27, 1, 2, (void*) NULL },
  /* 0x28 */ { "???", 0x28, 1, 2, (void*) NULL },
  /* 0x29 */ { "AND", 0x29, 2, 2, (void*) and_imm },
  /* 0x2a */ { "ROL", 0x2a, 1, 2, (void*) rol_acc },
  /* 0x2b */ { "???", 0x2b, 1, 2, (void*) NULL },
  /* 0x2c */ { "BIT", 0x2c, 3, 4, (void*) bit_abs },
  /* 0x2d */ { "AND", 0x2d, 3, 4, (void*) and_abs },
  /* 0x2e */ { "ROL", 0x2e, 3, 6, (void*) rol_abs },
  /* 0x2f */ { "???", 0x2f, 1, 2, (void*) NULL },
  /* 0x30 */ { "BMI", 0x30, 2, 2, (void*) bmi },
  /* 0x31 */ { "AND", 0x31, 2, 5, (void*) and_indy },
  /* 0x32 */ { "???", 0x32, 1, 2, (void*) NULL },
  /* 0x33 */ { "???", 0x33, 1, 2, (void*) NULL },
  /* 0x34 */ { "???", 0x34, 1, 2, (void*) NULL },
  /* 0x35 */ { "AND", 0x35, 2, 4, (void*) and_zpgx },
  /* 0x36 */ { "ROL", 0x36, 2, 6, (void*) rol_zpgx },
  /* 0x37 */ { "???", 0x37, 1, 2, (void*) NULL },
  /* 0x38 */ { "SEC", 0x38, 1, 2, (void*) sec },
  /* 0x39 */ { "AND", 0x39, 3, 4, (void*) and_absy },
  /* 0x3a */ { "???", 0x3a, 1, 2, (void*) NULL },
  /* 0x3b */ { "???", 0x3b, 1, 2, (void*) NULL },
  /* 0x3c */ { "???", 0x3c, 1, 2, (void*) NULL },
  /* 0x3d */ { "AND", 0x3d, 3, 4, (void*) and_absx },
  /* 0x3e */ { "ROL", 0x3e, 3, 7, (void*) rol_absx },
  /* 0x3f */ { "???", 0x3f, 1, 2, (void*) NULL },

  /* 0x40 */ { "RTI", 0x40, 1, 6, (void*) rti },
  /* 0x41 */ { "EOR", 0x41, 2, 6, (void*) eor_indx },
  /* 0x42 */ { "???", 0x42, 1, 2, (void*) NULL },
  /* 0x43 */ { "???", 0x43, 1, 2, (void*) NULL },
  /* 0x44 */ { "???", 0x44, 1, 2, (void*) NULL },
  /* 0x45 */ { "EOR", 0x45, 2, 3, (void*) eor_zpg },
  /* 0x46 */ { "LSR", 0x46, 2, 5, (void*) lsr_zpg },
  /* 0x47 */ { "???", 0x47, 1, 2, (void*) NULL },
  /* 0x48 */ { "PHA", 0x48, 1, 3, (void*) pha },
  /* 0x49 */ { "EOR", 0x49, 2, 2, (void*) eor_imm },
  /* 0x4a */ { "LSR", 0x4a, 1, 2, (void*) lsr_acc },
  /* 0x4b */ { "???", 0x4b, 1, 2, (void*) NULL },
  /* 0x4c */ { "JMP", 0x4c, 3, 3, (void*) jmp_abs },
  /* 0x4d */ { "EOR", 0x4d, 3, 4, (void*) eor_abs },
  /* 0x4e */ { "LSR", 0x4e, 3, 6, (void*) lsr_abs },
  /* 0x4f */ { "???", 0x4f, 1, 2, (void*) NULL },
  /* 0x50 */ { "BVC", 0x50, 2, 2, (void*) bvc },
  /* 0x51 */ { "EOR", 0x51, 2, 5, (void*) eor_indy },
  /* 0x52 */ { "???", 0x52, 1, 2, (void*) NULL },
  /* 0x53 */ { "???", 0x53, 1, 2, (void*) NULL },
  /* 0x54 */ { "???", 0x54, 1, 2, (void*) NULL },
  /* 0x55 */ { "EOR", 0x55, 2, 4, (void*) eor_zpgx },
  /* 0x56 */ { "LSR", 0x56, 2, 6, (void*) lsr_zpgx },
  /* 0x57 */ { "???", 0x57, 1, 2, (void*) NULL },
  /* 0x58 */ { "CLI", 0x58, 1, 2, (void*) cli },
  /* 0x59 */ { "EOR", 0x59, 3, 4, (void*) eor_absy },
  /* 0x5a */ { "???", 0x5a, 1, 2, (void*) NULL },
  /* 0x5b */ { "???", 0x5b, 1, 2, (void*) NULL },
  /* 0x5c */ { "???", 0x5c, 1, 2, (void*) NULL },
  /* 0x5d */ { "EOR", 0x5d, 3, 4, (void*) eor_absx },
  /* 0x5e */ { "LSR", 0x5e, 3, 7, (void*) lsr_absx },
  /* 0x5f */ { "???", 0x5f, 1, 2, (void*) NULL },

  /* 0x60 */ { "RTS", 0x60, 1, 6, (void*) rts },
  /* 0x61 */ { "ADC", 0x61, 2, 6, (void*) adc_indx },
  /* 0x62 */ { "???", 0x62, 1, 2, (void*) NULL },
  /* 0x63 */ { "???", 0x63, 1, 2, (void*) NULL },
  /* 0x64 */ { "???", 0x64, 1, 2, (void*) NULL },
  /* 0x65 */ { "ADC", 0x65, 2, 3, (void*) adc_zpg },
  /* 0x66 */ { "ROR", 0x66, 2, 5, (void*) ror_zpg },
  /* 0x67 */ { "???", 0x67, 1, 2, (void*) NULL },
  /* 0x68 */ { "PLA", 0x68, 1, 4, (void*) pla },
  /* 0x69 */ { "ADC", 0x69, 2, 2, (void*) adc_imm },
  /* 0x6a */ { "ROR", 0x6a, 1, 2, (void*) ror_acc },
  /* 0x6b */ { "???", 0x6b, 1, 2, (void*) NULL },
  /* 0x6c */ { "JMP", 0x6c, 3, 5, (void*) jmp_ind },
  /* 0x6d */ { "ADC", 0x6d, 3, 4, (void*) adc_abs },
  /* 0x6e */ { "ROR", 0x6e, 3, 6, (void*) ror_abs },
  /* 0x6f */ { "???", 0x6f, 1, 2, (void*) NULL },
  /* 0x70 */ { "BVS", 0x70, 2, 2, (void*) bvs },
  /* 0x71 */ { "ADC", 0x71, 2, 5, (void*) adc_indy },
  /* 0x72 */ { "???", 0x72, 1, 2, (void*) NULL },
  /* 0x73 */ { "???", 0x73, 1, 2, (void*) NULL },
  /* 0x74 */ { "???", 0x74, 1, 2, (void*) NULL },
  /* 0x75 */ { "ADC", 0x75, 2, 4, (void*) adc_zpgx },
  /* 0x76 */ { "ROR", 0x76, 2, 6, (void*) ror_zpgx },
  /* 0x77 */ { "???", 0x77, 1, 2, (void*) NULL },
  /* 0x78 */ { "SEI", 0x78, 1, 2, (void*) sei },
  /* 0x79 */ { "ADC", 0x79, 3, 4, (void*) adc_absy },
  /* 0x7a */ { "???", 0x7a, 1, 2, (void*) NULL },
  /* 0x7b */ { "???", 0x7b, 1, 2, (void*) NULL },
  /* 0x7c */ { "???", 0x7c, 1, 2, (void*) NULL },
  /* 0x7d */ { "ADC", 0x7d, 3, 4, (void*) adc_absx },
  /* 0x7e */ { "ROR", 0x7e, 3, 7, (void*) ror_absx },
  /* 0x7f */ { "???", 0x7f, 1, 2, (void*) NULL },

  /* 0x80 */ { "???", 0x80, 1, 2, (void*) NULL },
  /* 0x81 */ { "STA", 0x81, 2, 6, (void*) sta_indx },
  /* 0x82 */ { "???", 0x82, 1, 2, (void*) NULL },
  /* 0x83 */ { "???", 0x83, 1, 2, (void*) NULL },
  /* 0x84 */ { "STY", 0x84, 2, 3, (void*) sty_zpg },
  /* 0x85 */ { "STA", 0x85, 2, 3, (void*) sta_zpg },
  /* 0x86 */ { "STX", 0x86, 2, 3, (void*) stx_zpg },
  /* 0x87 */ { "???", 0x87, 1, 2, (void*) NULL },
  /* 0x88 */ { "DEY", 0x88, 1, 2, (void*) dey },
  /* 0x89 */ { "???", 0x89, 1, 2, (void*) NULL },
  /* 0x8a */ { "TXA", 0x8a, 1, 2, (void*) txa },
  /* 0x8b */ { "???", 0x8b, 1, 2, (void*) NULL },
  /* 0x8c */ { "STY", 0x8c, 3, 4, (void*) sty_abs },
  /* 0x8d */ { "STA", 0x8d, 3, 4, (void*) sta_abs },
  /* 0x8e */ { "STX", 0x8e, 3, 4, (void*) stx_abs },
  /* 0x8f */ { "???", 0x8f, 1, 2, (void*) NULL },
  /* 0x90 */ { "BCC", 0x90, 2, 2, (void*) bcc },
  /* 0x91 */ { "STA", 0x91, 2, 6, (void*) sta_indy },
  /* 0x92 */ { "???", 0x92, 1, 2, (void*) NULL },
  /* 0x93 */ { "???", 0x93, 1, 2, (void*) NULL },
  /* 0x94 */ { "STY", 0x94, 2, 4, (void*) sty_zpgx },
  /* 0x95 */ { "STA", 0x95, 2, 4, (void*) sta_zpgx },
  /* 0x96 */ { "STX", 0x96, 2, 4, (void*) stx_zpgy },
  /* 0x97 */ { "???", 0x97, 1, 2, (void*) NULL },
  /* 0x98 */ { "TYA", 0x98, 1, 2, (void*) tya },
  /* 0x99 */ { "STA", 0x99, 3, 5, (void*) sta_absy },
  /* 0x9a */ { "TXS", 0x9a, 1, 2, (void*) txs },
  /* 0x9b */ { "???", 0x9b, 1, 2, (void*) NULL },
  /* 0x9c */ { "???", 0x9c, 1, 2, (void*) NULL },
  /* 0x9d */ { "STA", 0x9d, 2, 5, (void*) sta_absx },
  /* 0x9e */ { "???", 0x9e, 1, 2, (void*) NULL },
  /* 0x9f */ { "???", 0x9f, 1, 2, (void*) NULL },

  /* 0xa0 */ { "LDY", 0xa0, 2, 2, (void*) ldy_imm },
  /* 0xa1 */ { "LDA", 0xa1, 2, 6, (void*) lda_indx },
  /* 0xa2 */ { "LDX", 0xa2, 2, 2, (void*) ldx_imm },
  /* 0xa3 */ { "???", 0xa3, 1, 2, (void*) NULL },
  /* 0xa4 */ { "LDY", 0xa4, 2, 3, (void*) ldy_zpg },
  /* 0xa5 */ { "LDA", 0xa5, 2, 3, (void*) lda_zpg },
  /* 0xa6 */ { "LDX", 0xa6, 2, 3, (void*) ldx_zpg },
  /* 0xa7 */ { "???", 0xa7, 1, 2, (void*) NULL },
  /* 0xa8 */ { "TAY", 0xa8, 1, 2, (void*) tay },
  /* 0xa9 */ { "LDA", 0xa9, 2, 2, (void*) lda_imm },
  /* 0xaa */ { "TAX", 0xaa, 1, 2, (void*) tax },
  /* 0xab */ { "???", 0xab, 1, 2, (void*) NULL },
  /* 0xac */ { "LDY", 0xac, 3, 4, (void*) ldy_abs },
  /* 0xad */ { "LDA", 0xad, 3, 4, (void*) lda_abs },
  /* 0xae */ { "LDX", 0xae, 3, 4, (void*) ldx_abs },
  /* 0xaf */ { "???", 0xaf, 1, 2, (void*) NULL },
  /* 0xb0 */ { "BCS", 0xb0, 2, 2, (void*) bcs },
  /* 0xb1 */ { "LDA", 0xb1, 2, 5, (void*) lda_indy },
  /* 0xb2 */ { "???", 0xb2, 1, 2, (void*) NULL },
  /* 0xb3 */ { "???", 0xb3, 1, 2, (void*) NULL },
  /* 0xb4 */ { "LDY", 0xb4, 2, 4, (void*) ldy_zpgx },
  /* 0xb5 */ { "LDA", 0xb5, 2, 4, (void*) lda_zpgx },
  /* 0xb6 */ { "LDX", 0xb6, 2, 4, (void*) ldx_zpgy },
  /* 0xb7 */ { "???", 0xb7, 1, 2, (void*) NULL },
  /* 0xb8 */ { "CLV", 0xb8, 1, 2, (void*) clv },
  /* 0xb9 */ { "LDA", 0xb9, 3, 4, (void*) lda_absy },
  /* 0xba */ { "TSX", 0xba, 1, 2, (void*) tsx },
  /* 0xbb */ { "???", 0xbb, 1, 2, (void*) NULL },
  /* 0xbc */ { "LDY", 0xbc, 3, 4, (void*) ldy_absx },
  /* 0xbd */ { "LDA", 0xbd, 3, 4, (void*) lda_absx },
  /* 0xbe */ { "LDX", 0xbe, 3, 4, (void*) ldx_absy },
  /* 0xbf */ { "???", 0xbf, 1, 2, (void*) NULL },

  /* 0xc0 */ { "CPY", 0xc0, 2, 2, (void*) cpy_imm },
  /* 0xc1 */ { "CMP", 0xc1, 2, 6, (void*) cmp_indx },
  /* 0xc2 */ { "???", 0xc2, 1, 2, (void*) NULL },
  /* 0xc3 */ { "???", 0xc3, 1, 2, (void*) NULL },
  /* 0xc4 */ { "CPY", 0xc4, 2, 3, (void*) cpy_zpg },
  /* 0xc5 */ { "CMP", 0xc5, 2, 3, (void*) cmp_zpg },
  /* 0xc6 */ { "DEC", 0xc6, 2, 5, (void*) dec_zpg },
  /* 0xc7 */ { "???", 0xc7, 1, 2, (void*) NULL },
  /* 0xc8 */ { "INY", 0xc8, 1, 2, (void*) iny },
  /* 0xc9 */ { "CMP", 0xc9, 2, 2, (void*) cmp_imm },
  /* 0xca */ { "DEX", 0xca, 1, 2, (void*) dex },
  /* 0xcb */ { "???", 0xcb, 1, 2, (void*) NULL },
  /* 0xcc */ { "CPY", 0xcc, 3, 4, (void*) cpy_abs },
  /* 0xcd */ { "CMP", 0xcd, 3, 4, (void*) cmp_abs },
  /* 0xce */ { "DEC", 0xce, 3, 3, (void*) dec_abs },
  /* 0xcf */ { "???", 0xcf, 1, 2, (void*) NULL },
  /* 0xd0 */ { "BNE", 0xd0, 2, 2, (void*) bne },
  /* 0xd1 */ { "CMP", 0xd1, 2, 5, (void*) cmp_indy },
  /* 0xd2 */ { "???", 0xd2, 1, 2, (void*) NULL },
  /* 0xd3 */ { "???", 0xd3, 1, 2, (void*) NULL },
  /* 0xd4 */ { "???", 0xd4, 1, 2, (void*) NULL },
  /* 0xd5 */ { "CMP", 0xd5, 2, 4, (void*) cmp_zpgx },
  /* 0xd6 */ { "DEC", 0xd6, 2, 6, (void*) dec_zpgx },
  /* 0xd7 */ { "???", 0xd7, 1, 2, (void*) NULL },
  /* 0xd8 */ { "CLD", 0xd8, 1, 2, (void*) cld },
  /* 0xd9 */ { "CMP", 0xd9, 3, 4, (void*) cmp_absy },
  /* 0xda */ { "???", 0xda, 1, 2, (void*) NULL },
  /* 0xdb */ { "???", 0xdb, 1, 2, (void*) NULL },
  /* 0xdc */ { "???", 0xdc, 1, 2, (void*) NULL },
  /* 0xdd */ { "CMP", 0xdd, 3, 4, (void*) cmp_absx },
  /* 0xde */ { "DEC", 0xde, 3, 7, (void*) dec_absx },
  /* 0xdf */ { "???", 0xdf, 1, 2, (void*) NULL },

  /* 0xe0 */ { "CPX", 0xe0, 2, 2, (void*) cpx_imm },
  /* 0xe1 */ { "SBC", 0xe1, 2, 2, (void*) sbc_indx },
  /* 0xe2 */ { "???", 0xe2, 1, 2, (void*) NULL },
  /* 0xe3 */ { "???", 0xe3, 1, 2, (void*) NULL },
  /* 0xe4 */ { "CPX", 0xe4, 2, 3, (void*) cpx_zpg },
  /* 0xe5 */ { "SBC", 0xe5, 2, 2, (void*) sbc_zpg },
  /* 0xe6 */ { "INC", 0xe6, 2, 5, (void*) inc_zpg },
  /* 0xe7 */ { "???", 0xe7, 1, 2, (void*) NULL },
  /* 0xe8 */ { "INX", 0xe8, 1, 2, (void*) inx },
  /* 0xe9 */ { "SBC", 0xe9, 2, 2, (void*) sbc_imm },
  /* 0xea */ { "NOP", 0xea, 1, 2, (void*) nop },
  /* 0xeb */ { "???", 0xeb, 1, 2, (void*) NULL },
  /* 0xec */ { "CPX", 0xec, 3, 4, (void*) cpx_abs },
  /* 0xed */ { "SBC", 0xed, 3, 2, (void*) sbc_abs },
  /* 0xee */ { "INC", 0xee, 3, 6, (void*) inc_abs },
  /* 0xef */ { "???", 0xef, 1, 2, (void*) NULL },
  /* 0xf0 */ { "BEQ", 0xf0, 2, 2, (void*) beq },
  /* 0xf1 */ { "SBC", 0xf1, 1, 2, (void*) sbc_indy },
  /* 0xf2 */ { "???", 0xf2, 1, 2, (void*) NULL },
  /* 0xf3 */ { "???", 0xf3, 1, 2, (void*) NULL },
  /* 0xf4 */ { "???", 0xf4, 1, 2, (void*) NULL },
  /* 0xf5 */ { "SBC", 0xf5, 1, 2, (void*) sbc_zpgx },
  /* 0xf6 */ { "INC", 0xf6, 2, 6, (void*) inc_zpgx },
  /* 0xf7 */ { "???", 0xf7, 1, 2, (void*) NULL },
  /* 0xf8 */ { "SED", 0xf8, 1, 2, (void*) sed },
  /* 0xf9 */ { "SBC", 0xf9, 3, 2, (void*) sbc_absy },
  /* 0xfa */ { "???", 0xfa, 1, 2, (void*) NULL },
  /* 0xfb */ { "???", 0xfb, 1, 2, (void*) NULL },
  /* 0xfc */ { "???", 0xfc, 1, 2, (void*) NULL },
  /* 0xfd */ { "SBC", 0xfd, 3, 2, (void*) sbc_absx },
  /* 0xfe */ { "INC", 0xfe, 3, 7, (void*) inc_absx },
  /* 0xff */ { "???", 0xff, 1, 2, (void*) NULL }
};
