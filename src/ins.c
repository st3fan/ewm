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

/* #if defined(EWM_LUA) */
/* #include <lua.h> */
/* #include <lauxlib.h> */
/* #include <lualib.h> */
/* #endif */

#include "ins.h"
#include "cpu.h"
#include "mem.h"
#if defined(EWM_LUA)
#include "lua.h"
#endif

static void update_zn(struct cpu_t *cpu, uint8_t v) {
  cpu->state.z = (v == 0x00);
  cpu->state.n = (v & 0x80);
}

// EWM_CPU_MODEL_6502

/* ADC */

static void adc(struct cpu_t *cpu, uint8_t m) {
   uint8_t c = cpu->state.c ? 1 : 0;
   if (cpu->state.d) {
      uint8_t cb = 0;

      uint8_t low = (cpu->state.a & 0x0f) + (m & 0x0f) + c;
      if ((low & 0xff) > 9) {
         low += 6;
      }
      if (low > 15) {
         cb = 1;
      }

      uint8_t high = (cpu->state.a >> 4) + (m >> 4) + cb;
      if ((high & 0xff) > 9) {
         high += 6;
      }
      uint8_t r = (low & 0x0F) | ((high<<4)&0xF0);

      cpu->state.c = (high > 15);
      cpu->state.z = (r == 0);
      cpu->state.n = (r & 0b10000000); // TODO Only on 6502? Does the 6502 test still pass?
      cpu->state.v = 0;

      cpu->state.a = r;
   } else {
      uint16_t t = (uint16_t)cpu->state.a + (uint16_t)m + (uint16_t)c;
      uint8_t r = (int8_t)t;
      cpu->state.c = (t & 0x0100) != 0;
      cpu->state.v = (cpu->state.a^r) & (m^r) & 0x80;
      cpu->state.a = r;
      update_zn(cpu, cpu->state.a);
   }
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
  if (cpu->model == EWM_CPU_MODEL_65C02) {
     cpu->state.d = 0;
  }
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

static void php(struct cpu_t *cpu) {
   _cpu_push_byte(cpu, _cpu_get_status(cpu));
}

static void plp(struct cpu_t *cpu) {
   _cpu_set_status(cpu, _cpu_pull_byte(cpu));
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
   uint8_t c = cpu->state.c ? 1 : 0;
   if (cpu->state.d) {
      uint8_t cb = 0;

      if (c == 0) {
         c = 1;
      } else {
         c = 0;
      }

      uint8_t low = (cpu->state.a & 0x0F) - (m & 0x0F) - c;
      if ((low & 0x10) != 0) {
         low -= 6;
      }
      if ((low & 0x10) != 0) {
         cb = 1;
      }

      uint8_t high = (cpu->state.a >> 4) - (m >> 4) - cb;
      if ((high & 0x10) != 0) {
         high -= 6;
      }

      int8_t result = (low & 0x0F) | (high << 4);

      cpu->state.c = (high & 0xff) < 15;
      cpu->state.z = (result == 0);
      cpu->state.n = (result & 0b10000000); // TODO Only on 6502? Does the 6502 test still pass?
      cpu->state.v = 0;

      cpu->state.a = result;
   } else {
      adc(cpu, m ^ 0xff);
   }
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

static void unimplemented(struct cpu_t *cpu) {
   // This is handled in cpu_execute_instruction() if strict mode is enabled.
}

/* Instruction dispatch table */

struct cpu_instruction_t instructions[256] = {
  /* 0x00 */ { "BRK", 0x00, 1, 2,  3, (void*) brk, -2, -2 },
  /* 0x01 */ { "ORA", 0x01, 2, 6,  0, (void*) ora_indx, -2, -2 },
  /* 0x02 */ { "???", 0x02, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x03 */ { "???", 0x03, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x04 */ { "???", 0x04, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x05 */ { "ORA", 0x05, 2, 2,  0, (void*) ora_zpg, -2, -2 },
  /* 0x06 */ { "ASL", 0x06, 2, 5,  0, (void*) asl_zpg, -2, -2 },
  /* 0x07 */ { "???", 0x07, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x08 */ { "PHP", 0x08, 1, 3,  0, (void*) php, -2, -2 },
  /* 0x09 */ { "ORA", 0x09, 2, 2,  0, (void*) ora_imm, -2, -2 },
  /* 0x0a */ { "ASL", 0x0a, 1, 2,  0, (void*) asl_acc, -2, -2 },
  /* 0x0b */ { "???", 0x0b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x0c */ { "???", 0x0c, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x0d */ { "ORA", 0x0d, 3, 4,  0, (void*) ora_abs, -2, -2 },
  /* 0x0e */ { "ASL", 0x0e, 3, 6,  0, (void*) asl_abs, -2, -2 },
  /* 0x0f */ { "???", 0x0f, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x10 */ { "BPL", 0x10, 2, 2,  0, (void*) bpl, -2, -2 },
  /* 0x11 */ { "ORA", 0x11, 2, 5,  0, (void*) ora_indy, -2, -2 },
  /* 0x12 */ { "???", 0x12, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x13 */ { "???", 0x13, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x14 */ { "???", 0x14, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x15 */ { "ORA", 0x15, 2, 3,  0, (void*) ora_zpgx, -2, -2 },
  /* 0x16 */ { "ASL", 0x16, 2, 6,  0, (void*) asl_zpgx, -2, -2 },
  /* 0x17 */ { "???", 0x17, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x18 */ { "CLC", 0x18, 1, 2,  0, (void*) clc, -2, -2 },
  /* 0x19 */ { "ORA", 0x19, 3, 4,  0, (void*) ora_absy, -2, -2 },
  /* 0x1a */ { "???", 0x1a, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x1b */ { "???", 0x1b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x1c */ { "???", 0x1c, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x1d */ { "ORA", 0x1d, 3, 4,  0, (void*) ora_absx, -2, -2 },
  /* 0x1e */ { "ASL", 0x1e, 3, 7,  0, (void*) asl_absx, -2, -2 },
  /* 0x1f */ { "???", 0x1f, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0x20 */ { "JSR", 0x20, 3, 6,  2, (void*) jsr_abs, -2, -2 },
  /* 0x21 */ { "AND", 0x21, 2, 6,  0, (void*) and_indx, -2, -2 },
  /* 0x22 */ { "???", 0x22, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x23 */ { "???", 0x23, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x24 */ { "BIT", 0x24, 2, 3,  0, (void*) bit_zpg, -2, -2 },
  /* 0x25 */ { "AND", 0x25, 2, 3,  0, (void*) and_zpg, -2, -2 },
  /* 0x26 */ { "ROL", 0x26, 2, 5,  0, (void*) rol_zpg, -2, -2 },
  /* 0x27 */ { "???", 0x27, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x28 */ { "PLP", 0x28, 1, 4,  0, (void*) plp, -2, -2 },
  /* 0x29 */ { "AND", 0x29, 2, 2,  0, (void*) and_imm, -2, -2 },
  /* 0x2a */ { "ROL", 0x2a, 1, 2,  0, (void*) rol_acc, -2, -2 },
  /* 0x2b */ { "???", 0x2b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x2c */ { "BIT", 0x2c, 3, 4,  0, (void*) bit_abs, -2, -2 },
  /* 0x2d */ { "AND", 0x2d, 3, 4,  0, (void*) and_abs, -2, -2 },
  /* 0x2e */ { "ROL", 0x2e, 3, 6,  0, (void*) rol_abs, -2, -2 },
  /* 0x2f */ { "???", 0x2f, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x30 */ { "BMI", 0x30, 2, 2,  0, (void*) bmi, -2, -2 },
  /* 0x31 */ { "AND", 0x31, 2, 5,  0, (void*) and_indy, -2, -2 },
  /* 0x32 */ { "???", 0x32, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x33 */ { "???", 0x33, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x34 */ { "???", 0x34, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x35 */ { "AND", 0x35, 2, 4,  0, (void*) and_zpgx, -2, -2 },
  /* 0x36 */ { "ROL", 0x36, 2, 6,  0, (void*) rol_zpgx, -2, -2 },
  /* 0x37 */ { "???", 0x37, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x38 */ { "SEC", 0x38, 1, 2,  0, (void*) sec, -2, -2 },
  /* 0x39 */ { "AND", 0x39, 3, 4,  0, (void*) and_absy, -2, -2 },
  /* 0x3a */ { "???", 0x3a, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x3b */ { "???", 0x3b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x3c */ { "???", 0x3c, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x3d */ { "AND", 0x3d, 3, 4,  0, (void*) and_absx, -2, -2 },
  /* 0x3e */ { "ROL", 0x3e, 3, 7,  0, (void*) rol_absx, -2, -2 },
  /* 0x3f */ { "???", 0x3f, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0x40 */ { "RTI", 0x40, 1, 6, -3, (void*) rti, -2, -2 },
  /* 0x41 */ { "EOR", 0x41, 2, 6,  0, (void*) eor_indx, -2, -2 },
  /* 0x42 */ { "???", 0x42, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x43 */ { "???", 0x43, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x44 */ { "???", 0x44, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x45 */ { "EOR", 0x45, 2, 3,  0, (void*) eor_zpg, -2, -2 },
  /* 0x46 */ { "LSR", 0x46, 2, 5,  0, (void*) lsr_zpg, -2, -2 },
  /* 0x47 */ { "???", 0x47, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x48 */ { "PHA", 0x48, 1, 3,  1, (void*) pha, -2, -2 },
  /* 0x49 */ { "EOR", 0x49, 2, 2,  0, (void*) eor_imm, -2, -2 },
  /* 0x4a */ { "LSR", 0x4a, 1, 2,  0, (void*) lsr_acc, -2, -2 },
  /* 0x4b */ { "???", 0x4b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x4c */ { "JMP", 0x4c, 3, 3,  0, (void*) jmp_abs, -2, -2 },
  /* 0x4d */ { "EOR", 0x4d, 3, 4,  0, (void*) eor_abs, -2, -2 },
  /* 0x4e */ { "LSR", 0x4e, 3, 6,  0, (void*) lsr_abs, -2, -2 },
  /* 0x4f */ { "???", 0x4f, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x50 */ { "BVC", 0x50, 2, 2,  0, (void*) bvc, -2, -2 },
  /* 0x51 */ { "EOR", 0x51, 2, 5,  0, (void*) eor_indy, -2, -2 },
  /* 0x52 */ { "???", 0x52, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x53 */ { "???", 0x53, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x54 */ { "???", 0x54, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x55 */ { "EOR", 0x55, 2, 4,  0, (void*) eor_zpgx, -2, -2 },
  /* 0x56 */ { "LSR", 0x56, 2, 6,  0, (void*) lsr_zpgx, -2, -2 },
  /* 0x57 */ { "???", 0x57, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x58 */ { "CLI", 0x58, 1, 2,  0, (void*) cli, -2, -2 },
  /* 0x59 */ { "EOR", 0x59, 3, 4,  0, (void*) eor_absy, -2, -2 },
  /* 0x5a */ { "???", 0x5a, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x5b */ { "???", 0x5b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x5c */ { "???", 0x5c, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x5d */ { "EOR", 0x5d, 3, 4,  0, (void*) eor_absx, -2, -2 },
  /* 0x5e */ { "LSR", 0x5e, 3, 7,  0, (void*) lsr_absx, -2, -2 },
  /* 0x5f */ { "???", 0x5f, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0x60 */ { "RTS", 0x60, 1, 6, -2, (void*) rts, -2, -2 },
  /* 0x61 */ { "ADC", 0x61, 2, 6,  0, (void*) adc_indx, -2, -2 },
  /* 0x62 */ { "???", 0x62, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x63 */ { "???", 0x63, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x64 */ { "???", 0x64, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x65 */ { "ADC", 0x65, 2, 3,  0, (void*) adc_zpg, -2, -2 },
  /* 0x66 */ { "ROR", 0x66, 2, 5,  0, (void*) ror_zpg, -2, -2 },
  /* 0x67 */ { "???", 0x67, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x68 */ { "PLA", 0x68, 1, 4, -1, (void*) pla, -2, -2 },
  /* 0x69 */ { "ADC", 0x69, 2, 2,  0, (void*) adc_imm, -2, -2 },
  /* 0x6a */ { "ROR", 0x6a, 1, 2,  0, (void*) ror_acc, -2, -2 },
  /* 0x6b */ { "???", 0x6b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x6c */ { "JMP", 0x6c, 3, 5,  0, (void*) jmp_ind, -2, -2 },
  /* 0x6d */ { "ADC", 0x6d, 3, 4,  0, (void*) adc_abs, -2, -2 },
  /* 0x6e */ { "ROR", 0x6e, 3, 6,  0, (void*) ror_abs, -2, -2 },
  /* 0x6f */ { "???", 0x6f, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x70 */ { "BVS", 0x70, 2, 2,  0, (void*) bvs, -2, -2 },
  /* 0x71 */ { "ADC", 0x71, 2, 5,  0, (void*) adc_indy, -2, -2 },
  /* 0x72 */ { "???", 0x72, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x73 */ { "???", 0x73, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x74 */ { "???", 0x74, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x75 */ { "ADC", 0x75, 2, 4,  0, (void*) adc_zpgx, -2, -2 },
  /* 0x76 */ { "ROR", 0x76, 2, 6,  0, (void*) ror_zpgx, -2, -2 },
  /* 0x77 */ { "???", 0x77, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x78 */ { "SEI", 0x78, 1, 2,  0, (void*) sei, -2, -2 },
  /* 0x79 */ { "ADC", 0x79, 3, 4,  0, (void*) adc_absy, -2, -2 },
  /* 0x7a */ { "???", 0x7a, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x7b */ { "???", 0x7b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x7c */ { "???", 0x7c, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x7d */ { "ADC", 0x7d, 3, 4,  0, (void*) adc_absx, -2, -2 },
  /* 0x7e */ { "ROR", 0x7e, 3, 7,  0, (void*) ror_absx, -2, -2 },
  /* 0x7f */ { "???", 0x7f, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0x80 */ { "???", 0x80, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x81 */ { "STA", 0x81, 2, 6,  0, (void*) sta_indx, -2, -2 },
  /* 0x82 */ { "???", 0x82, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x83 */ { "???", 0x83, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x84 */ { "STY", 0x84, 2, 3,  0, (void*) sty_zpg, -2, -2 },
  /* 0x85 */ { "STA", 0x85, 2, 3,  0, (void*) sta_zpg, -2, -2 },
  /* 0x86 */ { "STX", 0x86, 2, 3,  0, (void*) stx_zpg, -2, -2 },
  /* 0x87 */ { "???", 0x87, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x88 */ { "DEY", 0x88, 1, 2,  0, (void*) dey, -2, -2 },
  /* 0x89 */ { "???", 0x89, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x8a */ { "TXA", 0x8a, 1, 2,  0, (void*) txa, -2, -2 },
  /* 0x8b */ { "???", 0x8b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x8c */ { "STY", 0x8c, 3, 4,  0, (void*) sty_abs, -2, -2 },
  /* 0x8d */ { "STA", 0x8d, 3, 4,  0, (void*) sta_abs, -2, -2 },
  /* 0x8e */ { "STX", 0x8e, 3, 4,  0, (void*) stx_abs, -2, -2 },
  /* 0x8f */ { "???", 0x8f, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x90 */ { "BCC", 0x90, 2, 2,  0, (void*) bcc, -2, -2 },
  /* 0x91 */ { "STA", 0x91, 2, 6,  0, (void*) sta_indy, -2, -2 },
  /* 0x92 */ { "???", 0x92, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x93 */ { "???", 0x93, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x94 */ { "STY", 0x94, 2, 4,  0, (void*) sty_zpgx, -2, -2 },
  /* 0x95 */ { "STA", 0x95, 2, 4,  0, (void*) sta_zpgx, -2, -2 },
  /* 0x96 */ { "STX", 0x96, 2, 4,  0, (void*) stx_zpgy, -2, -2 },
  /* 0x97 */ { "???", 0x97, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x98 */ { "TYA", 0x98, 1, 2,  0, (void*) tya, -2, -2 },
  /* 0x99 */ { "STA", 0x99, 3, 5,  0, (void*) sta_absy, -2, -2 },
  /* 0x9a */ { "TXS", 0x9a, 1, 2,  0, (void*) txs, -2, -2 },
  /* 0x9b */ { "???", 0x9b, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x9c */ { "???", 0x9c, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x9d */ { "STA", 0x9d, 3, 5,  0, (void*) sta_absx, -2, -2 },
  /* 0x9e */ { "???", 0x9e, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0x9f */ { "???", 0x9f, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0xa0 */ { "LDY", 0xa0, 2, 2,  0, (void*) ldy_imm, -2, -2 },
  /* 0xa1 */ { "LDA", 0xa1, 2, 6,  0, (void*) lda_indx, -2, -2 },
  /* 0xa2 */ { "LDX", 0xa2, 2, 2,  0, (void*) ldx_imm, -2, -2 },
  /* 0xa3 */ { "???", 0xa3, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xa4 */ { "LDY", 0xa4, 2, 3,  0, (void*) ldy_zpg, -2, -2 },
  /* 0xa5 */ { "LDA", 0xa5, 2, 3,  0, (void*) lda_zpg, -2, -2 },
  /* 0xa6 */ { "LDX", 0xa6, 2, 3,  0, (void*) ldx_zpg, -2, -2 },
  /* 0xa7 */ { "???", 0xa7, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xa8 */ { "TAY", 0xa8, 1, 2,  0, (void*) tay, -2, -2 },
  /* 0xa9 */ { "LDA", 0xa9, 2, 2,  0, (void*) lda_imm, -2, -2 },
  /* 0xaa */ { "TAX", 0xaa, 1, 2,  0, (void*) tax, -2, -2 },
  /* 0xab */ { "???", 0xab, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xac */ { "LDY", 0xac, 3, 4,  0, (void*) ldy_abs, -2, -2 },
  /* 0xad */ { "LDA", 0xad, 3, 4,  0, (void*) lda_abs, -2, -2 },
  /* 0xae */ { "LDX", 0xae, 3, 4,  0, (void*) ldx_abs, -2, -2 },
  /* 0xaf */ { "???", 0xaf, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xb0 */ { "BCS", 0xb0, 2, 2,  0, (void*) bcs, -2, -2 },
  /* 0xb1 */ { "LDA", 0xb1, 2, 5,  0, (void*) lda_indy, -2, -2 },
  /* 0xb2 */ { "???", 0xb2, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xb3 */ { "???", 0xb3, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xb4 */ { "LDY", 0xb4, 2, 4,  0, (void*) ldy_zpgx, -2, -2 },
  /* 0xb5 */ { "LDA", 0xb5, 2, 4,  0, (void*) lda_zpgx, -2, -2 },
  /* 0xb6 */ { "LDX", 0xb6, 2, 4,  0, (void*) ldx_zpgy, -2, -2 },
  /* 0xb7 */ { "???", 0xb7, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xb8 */ { "CLV", 0xb8, 1, 2,  0, (void*) clv, -2, -2 },
  /* 0xb9 */ { "LDA", 0xb9, 3, 4,  0, (void*) lda_absy, -2, -2 },
  /* 0xba */ { "TSX", 0xba, 1, 2,  0, (void*) tsx, -2, -2 },
  /* 0xbb */ { "???", 0xbb, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xbc */ { "LDY", 0xbc, 3, 4,  0, (void*) ldy_absx, -2, -2 },
  /* 0xbd */ { "LDA", 0xbd, 3, 4,  0, (void*) lda_absx, -2, -2 },
  /* 0xbe */ { "LDX", 0xbe, 3, 4,  0, (void*) ldx_absy, -2, -2 },
  /* 0xbf */ { "???", 0xbf, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0xc0 */ { "CPY", 0xc0, 2, 2,  0, (void*) cpy_imm, -2, -2 },
  /* 0xc1 */ { "CMP", 0xc1, 2, 6,  0, (void*) cmp_indx, -2, -2 },
  /* 0xc2 */ { "???", 0xc2, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xc3 */ { "???", 0xc3, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xc4 */ { "CPY", 0xc4, 2, 3,  0, (void*) cpy_zpg, -2, -2 },
  /* 0xc5 */ { "CMP", 0xc5, 2, 3,  0, (void*) cmp_zpg, -2, -2 },
  /* 0xc6 */ { "DEC", 0xc6, 2, 5,  0, (void*) dec_zpg, -2, -2 },
  /* 0xc7 */ { "???", 0xc7, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xc8 */ { "INY", 0xc8, 1, 2,  0, (void*) iny, -2, -2 },
  /* 0xc9 */ { "CMP", 0xc9, 2, 2,  0, (void*) cmp_imm, -2, -2 },
  /* 0xca */ { "DEX", 0xca, 1, 2,  0, (void*) dex, -2, -2 },
  /* 0xcb */ { "???", 0xcb, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xcc */ { "CPY", 0xcc, 3, 4,  0, (void*) cpy_abs, -2, -2 },
  /* 0xcd */ { "CMP", 0xcd, 3, 4,  0, (void*) cmp_abs, -2, -2 },
  /* 0xce */ { "DEC", 0xce, 3, 3,  0, (void*) dec_abs, -2, -2 },
  /* 0xcf */ { "???", 0xcf, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xd0 */ { "BNE", 0xd0, 2, 2,  0, (void*) bne, -2, -2 },
  /* 0xd1 */ { "CMP", 0xd1, 2, 5,  0, (void*) cmp_indy, -2, -2 },
  /* 0xd2 */ { "???", 0xd2, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xd3 */ { "???", 0xd3, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xd4 */ { "???", 0xd4, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xd5 */ { "CMP", 0xd5, 2, 4,  0, (void*) cmp_zpgx, -2, -2 },
  /* 0xd6 */ { "DEC", 0xd6, 2, 6,  0, (void*) dec_zpgx, -2, -2 },
  /* 0xd7 */ { "???", 0xd7, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xd8 */ { "CLD", 0xd8, 1, 2,  0, (void*) cld, -2, -2 },
  /* 0xd9 */ { "CMP", 0xd9, 3, 4,  0, (void*) cmp_absy, -2, -2 },
  /* 0xda */ { "???", 0xda, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xdb */ { "???", 0xdb, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xdc */ { "???", 0xdc, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xdd */ { "CMP", 0xdd, 3, 4,  0, (void*) cmp_absx, -2, -2 },
  /* 0xde */ { "DEC", 0xde, 3, 7,  0, (void*) dec_absx, -2, -2 },
  /* 0xdf */ { "???", 0xdf, 1, 2,  0, (void*) unimplemented, -2, -2 },

  /* 0xe0 */ { "CPX", 0xe0, 2, 2,  0, (void*) cpx_imm, -2, -2 },
  /* 0xe1 */ { "SBC", 0xe1, 2, 2,  0, (void*) sbc_indx, -2, -2 },
  /* 0xe2 */ { "???", 0xe2, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xe3 */ { "???", 0xe3, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xe4 */ { "CPX", 0xe4, 2, 3,  0, (void*) cpx_zpg, -2, -2 },
  /* 0xe5 */ { "SBC", 0xe5, 2, 2,  0, (void*) sbc_zpg, -2, -2 },
  /* 0xe6 */ { "INC", 0xe6, 2, 5,  0, (void*) inc_zpg, -2, -2 },
  /* 0xe7 */ { "???", 0xe7, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xe8 */ { "INX", 0xe8, 1, 2,  0, (void*) inx, -2, -2 },
  /* 0xe9 */ { "SBC", 0xe9, 2, 2,  0, (void*) sbc_imm, -2, -2 },
  /* 0xea */ { "NOP", 0xea, 1, 2,  0, (void*) nop, -2, -2 },
  /* 0xeb */ { "???", 0xeb, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xec */ { "CPX", 0xec, 3, 4,  0, (void*) cpx_abs, -2, -2 },
  /* 0xed */ { "SBC", 0xed, 3, 2,  0, (void*) sbc_abs, -2, -2 },
  /* 0xee */ { "INC", 0xee, 3, 6,  0, (void*) inc_abs, -2, -2 },
  /* 0xef */ { "???", 0xef, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xf0 */ { "BEQ", 0xf0, 2, 2,  0, (void*) beq, -2, -2 },
  /* 0xf1 */ { "SBC", 0xf1, 2, 2,  0, (void*) sbc_indy, -2, -2 },
  /* 0xf2 */ { "???", 0xf2, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xf3 */ { "???", 0xf3, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xf4 */ { "???", 0xf4, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xf5 */ { "SBC", 0xf5, 2, 2,  0, (void*) sbc_zpgx, -2, -2 },
  /* 0xf6 */ { "INC", 0xf6, 2, 6,  0, (void*) inc_zpgx, -2, -2 },
  /* 0xf7 */ { "???", 0xf7, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xf8 */ { "SED", 0xf8, 1, 2,  0, (void*) sed, -2, -2 },
  /* 0xf9 */ { "SBC", 0xf9, 3, 2,  0, (void*) sbc_absy, -2, -2 },
  /* 0xfa */ { "???", 0xfa, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xfb */ { "???", 0xfb, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xfc */ { "???", 0xfc, 1, 2,  0, (void*) unimplemented, -2, -2 },
  /* 0xfd */ { "SBC", 0xfd, 3, 2,  0, (void*) sbc_absx, -2, -2 },
  /* 0xfe */ { "INC", 0xfe, 3, 7,  0, (void*) inc_absx, -2, -2 },
  /* 0xff */ { "???", 0xff, 1, 2,  0, (void*) unimplemented, -2, -2 }
};

// EWM_CPU_MODEL_65C02

static void ora_ind(struct cpu_t *cpu, uint8_t oper) {
   ora(cpu, mem_get_byte_ind(cpu, oper));
}

static void and_ind(struct cpu_t *cpu, uint8_t oper) {
   and(cpu, mem_get_byte_ind(cpu, oper));
}

static void eor_ind(struct cpu_t *cpu, uint8_t oper) {
   eor(cpu, mem_get_byte_ind(cpu, oper));
}

static void adc_ind(struct cpu_t *cpu, uint8_t oper) {
   adc(cpu, mem_get_byte_ind(cpu, oper));
}

static void sta_ind(struct cpu_t *cpu, uint8_t oper) {
   mem_set_byte_ind(cpu, oper, cpu->state.a);
}

static void lda_ind(struct cpu_t *cpu, uint8_t oper) {
   cpu->state.a = mem_get_byte_ind(cpu, oper);
   update_zn(cpu, cpu->state.a);
}

static void cmp_ind(struct cpu_t *cpu, uint8_t oper) {
   cmp(cpu, mem_get_byte_ind(cpu, oper));
}

static void sbc_ind(struct cpu_t *cpu, uint8_t oper) {
   sbc(cpu, mem_get_byte_ind(cpu, oper));
}

static void bit_imm(struct cpu_t *cpu, uint8_t oper) {
  uint8_t t = cpu->state.a & oper;
  cpu->state.z = (t == 0);
}

static void bit_zpgx(struct cpu_t *cpu, uint8_t oper) {
   bit(cpu, mem_get_byte_zpgx(cpu, oper));
}

static void bit_absx(struct cpu_t *cpu, uint16_t oper) {
   bit(cpu, mem_get_byte_absx(cpu, oper));
}

static void dec_acc(struct cpu_t *cpu) {
   cpu->state.a--;
   update_zn(cpu, cpu->state.a);
}

static void inc_acc(struct cpu_t *cpu) {
   cpu->state.a++;
   update_zn(cpu, cpu->state.a);
}

static void jmp_absx(struct cpu_t *cpu, uint16_t oper) {
   cpu->state.pc = mem_get_word(cpu, oper + cpu->state.x);
}

static void bra(struct cpu_t *cpu, uint8_t oper) {
   cpu->state.pc += (int8_t) oper;
}

static void phx(struct cpu_t *cpu) {
   _cpu_push_byte(cpu, cpu->state.x);
}

static void phy(struct cpu_t *cpu) {
   _cpu_push_byte(cpu, cpu->state.y);
}

static void plx(struct cpu_t *cpu) {
  cpu->state.x = _cpu_pull_byte(cpu);
  update_zn(cpu, cpu->state.x);
}

static void ply(struct cpu_t *cpu) {
  cpu->state.y = _cpu_pull_byte(cpu);
  update_zn(cpu, cpu->state.y);
}

static void stz_zpg(struct cpu_t *cpu, uint8_t oper) {
   mem_set_byte_zpg(cpu, oper, 0x00);
}

static void stz_zpgx(struct cpu_t *cpu, uint8_t oper) {
   mem_set_byte_zpgx(cpu, oper, 0x00);
}

static void stz_abs(struct cpu_t *cpu, uint16_t oper) {
   mem_set_byte_abs(cpu, oper, 0x00);
}

static void stz_absx(struct cpu_t *cpu, uint16_t oper) {
   mem_set_byte_absx(cpu, oper, 0x00);
}

static void trb_zpg(struct cpu_t *cpu, uint8_t oper) {
   cpu->state.z = (mem_get_byte(cpu, oper) & cpu->state.a) == 0;
   uint8_t r = mem_get_byte(cpu, oper) & ~cpu->state.a;
   mem_set_byte_zpg(cpu, oper, r);
}

static void trb_abs(struct cpu_t *cpu, uint16_t oper) {
   cpu->state.z = (mem_get_byte(cpu, oper) & cpu->state.a) == 0;
   uint8_t r = mem_get_byte(cpu, oper) & (cpu->state.a ^ 0xff);
   mem_set_byte_abs(cpu, oper, r);
}

static void tsb_zpg(struct cpu_t *cpu, uint8_t oper) {
   cpu->state.z = (mem_get_byte(cpu, oper) & cpu->state.a) == 0;
   uint8_t r = mem_get_byte(cpu, oper) | cpu->state.a;
   mem_set_byte_zpg(cpu, oper, r);
}

static void tsb_abs(struct cpu_t *cpu, uint16_t oper) {
   cpu->state.z = (mem_get_byte(cpu, oper) & cpu->state.a) == 0;
   uint8_t r = mem_get_byte(cpu, oper) | cpu->state.a;
   mem_set_byte_abs(cpu, oper, r);
}

static void bbr(struct cpu_t *cpu, uint8_t bit, uint8_t zp, int8_t label) {
   if ((mem_get_byte_zpg(cpu, zp) & bit) == 0) {
      cpu->state.pc += label;
   }
}

static void bbr0(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b00000001, oper & 0x00ff, oper >> 8);
}

static void bbr1(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b00000010, oper & 0x00ff, oper >> 8);
}

static void bbr2(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b00000100, oper & 0x00ff, oper >> 8);
}

static void bbr3(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b00001000, oper & 0x00ff, oper >> 8);
}

static void bbr4(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b00010000, oper & 0x00ff, oper >> 8);
}

static void bbr5(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b00100000, oper & 0x00ff, oper >> 8);
}

static void bbr6(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b01000000, oper & 0x00ff, oper >> 8);
}

static void bbr7(struct cpu_t *cpu, uint16_t oper) {
   bbr(cpu, 0b10000000, oper & 0x00ff, oper >> 8);
}

static void bbs(struct cpu_t *cpu, uint8_t bit, uint8_t zp, int8_t label) {
   if ((mem_get_byte_zpg(cpu, zp) & bit) != 0) {
      cpu->state.pc += label;
   }
}

static void bbs0(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b00000001, oper & 0x00ff, oper >> 8);
}

static void bbs1(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b00000010, oper & 0x00ff, oper >> 8);
}

static void bbs2(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b00000100, oper & 0x00ff, oper >> 8);
}

static void bbs3(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b00001000, oper & 0x00ff, oper >> 8);
}

static void bbs4(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b00010000, oper & 0x00ff, oper >> 8);
}

static void bbs5(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b00100000, oper & 0x00ff, oper >> 8);
}

static void bbs6(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b01000000, oper & 0x00ff, oper >> 8);
}

static void bbs7(struct cpu_t *cpu, uint16_t oper) {
   bbs(cpu, 0b10000000, oper & 0x00ff, oper >> 8);
}

static void rmb(struct cpu_t *cpu, uint8_t bit, uint8_t zp) {
   mem_set_byte_zpg(cpu, zp, mem_get_byte(cpu, zp) & ~bit);
}

static void rmb0(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b00000001, oper);
}

static void rmb1(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b00000010, oper);
}

static void rmb2(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b00000100, oper);
}

static void rmb3(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b00001000, oper);
}

static void rmb4(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b00010000, oper);
}

static void rmb5(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b00100000, oper);
}

static void rmb6(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b01000000, oper);
}

static void rmb7(struct cpu_t *cpu, uint8_t oper) {
   rmb(cpu, 0b10000000, oper);
}

static void smb(struct cpu_t *cpu, uint8_t bit, uint8_t zp) {
   mem_set_byte_zpg(cpu, zp, mem_get_byte(cpu, zp) | bit);
}

static void smb0(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b00000001, oper);
}

static void smb1(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b00000010, oper);
}

static void smb2(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b00000100, oper);
}

static void smb3(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b00001000, oper);
}

static void smb4(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b00010000, oper);
}

static void smb5(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b00100000, oper);
}

static void smb6(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b01000000, oper);
}

static void smb7(struct cpu_t *cpu, uint8_t oper) {
   smb(cpu, 0b10000000, oper);
}

/* Instruction dispatch table */

struct cpu_instruction_t instructions_65C02[256] = {
  /* 0x00 */ { "???", 0x00, 0, 0,  0, NULL, -2, -2 },
  /* 0x01 */ { "???", 0x01, 0, 0,  0, NULL, -2, -2 },
  /* 0x02 */ { "NOP", 0x02, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0x03 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x04 */ { "TSB", 0x04, 2, 5,  0, (void*) tsb_zpg, -2, -2 },
  /* 0x05 */ { "???", 0x05, 0, 0,  0, NULL, -2, -2 },
  /* 0x06 */ { "???", 0x06, 0, 0,  0, NULL, -2, -2 },
  /* 0x07 */ { "RMB", 0x07, 2, 5,  0, (void*) rmb0, -2, -2 },
  /* 0x08 */ { "???", 0x08, 0, 0,  0, NULL, -2, -2 },
  /* 0x09 */ { "???", 0x09, 0, 0,  0, NULL, -2, -2 },
  /* 0x0a */ { "???", 0x0a, 0, 0,  0, NULL, -2, -2 },
  /* 0x0b */ { "NOP", 0x0b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x0c */ { "TSB", 0x0c, 3, 6,  0, (void*) tsb_abs, -2, -2 },
  /* 0x0d */ { "???", 0x0d, 0, 0,  0, NULL, -2, -2 },
  /* 0x0e */ { "???", 0x0e, 0, 0,  0, NULL, -2, -2 },
  /* 0x0f */ { "BBR", 0x0f, 3, 5,  0, (void*) bbr0, -2, -2 },

  /* 0x10 */ { "???", 0x10, 0, 0,  0, NULL, -2, -2 },
  /* 0x11 */ { "???", 0x12, 0, 0,  0, NULL, -2, -2 },
  /* 0x12 */ { "ORA", 0x12, 2, 5,  0, (void*) ora_ind, -2, -2 },
  /* 0x13 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x14 */ { "TRB", 0x14, 2, 5,  0, (void*) trb_zpg, -2, -2 },
  /* 0x15 */ { "???", 0x15, 0, 0,  0, NULL, -2, -2 },
  /* 0x16 */ { "???", 0x16, 0, 0,  0, NULL, -2, -2 },
  /* 0x17 */ { "RMB", 0x17, 2, 5,  0, (void*) rmb1, -2, -2 },
  /* 0x18 */ { "???", 0x18, 0, 0,  0, NULL, -2, -2 },
  /* 0x19 */ { "???", 0x19, 0, 0,  0, NULL, -2, -2 },
  /* 0x1a */ { "INC", 0x1a, 1, 2,  0, (void*) inc_acc, -2, -2 },
  /* 0x1b */ { "NOP", 0x1b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x1c */ { "TRB", 0x1c, 3, 6,  0, (void*) trb_abs, -2, -2 },
  /* 0x1d */ { "???", 0x1d, 0, 0,  0, NULL, -2, -2 },
  /* 0x1e */ { "???", 0x1e, 0, 0,  0, NULL, -2, -2 },
  /* 0x1f */ { "BBR", 0x1f, 3, 5,  0, (void*) bbr1, -2, -2 },

  /* 0x20 */ { "???", 0x20, 0, 0,  0, NULL, -2, -2 },
  /* 0x21 */ { "???", 0x21, 0, 0,  0, NULL, -2, -2 },
  /* 0x22 */ { "NOP", 0x22, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0x23 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x24 */ { "???", 0x24, 0, 0,  0, NULL, -2, -2 },
  /* 0x25 */ { "???", 0x25, 0, 0,  0, NULL, -2, -2 },
  /* 0x26 */ { "???", 0x26, 0, 0,  0, NULL, -2, -2 },
  /* 0x27 */ { "RMB", 0x27, 2, 5,  0, (void*) rmb2, -2, -2 },
  /* 0x28 */ { "???", 0x28, 0, 0,  0, NULL, -2, -2 },
  /* 0x29 */ { "???", 0x29, 0, 0,  0, NULL, -2, -2 },
  /* 0x2a */ { "???", 0x2a, 0, 0,  0, NULL, -2, -2 },
  /* 0x2b */ { "NOP", 0x2b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x2c */ { "???", 0x2c, 0, 0,  0, NULL, -2, -2 },
  /* 0x2d */ { "???", 0x2d, 0, 0,  0, NULL, -2, -2 },
  /* 0x2e */ { "???", 0x2e, 0, 0,  0, NULL, -2, -2 },
  /* 0x2f */ { "BBR", 0x2f, 3, 5,  0, (void*) bbr2, -2, -2 },

  /* 0x30 */ { "???", 0x30, 0, 0,  0, NULL, -2, -2 },
  /* 0x31 */ { "???", 0x31, 0, 0,  0, NULL, -2, -2 },
  /* 0x32 */ { "AND", 0x32, 2, 5,  0, (void*) and_ind, -2, -2 },
  /* 0x33 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x34 */ { "BIT", 0x34, 2, 4,  0, (void*) bit_zpgx, -2, -2 },
  /* 0x35 */ { "???", 0x35, 0, 0,  0, NULL, -2, -2 },
  /* 0x36 */ { "???", 0x36, 0, 0,  0, NULL, -2, -2 },
  /* 0x37 */ { "RMB", 0x37, 2, 5,  0, (void*) rmb3, -2, -2 },
  /* 0x38 */ { "???", 0x38, 0, 0,  0, NULL, -2, -2 },
  /* 0x39 */ { "???", 0x39, 0, 0,  0, NULL, -2, -2 },
  /* 0x3a */ { "DEC", 0x3a, 1, 2,  0, (void*) dec_acc, -2, -2 },
  /* 0x3b */ { "NOP", 0x3b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x3c */ { "BIT", 0x3c, 3, 4,  0, (void*) bit_absx, -2, -2 },
  /* 0x3d */ { "???", 0x3d, 0, 0,  0, NULL, -2, -2 },
  /* 0x3e */ { "???", 0x3e, 0, 0,  0, NULL, -2, -2 },
  /* 0x3f */ { "BBR", 0x3f, 3, 5,  0, (void*) bbr3, -2, -2 },

  /* 0x40 */ { "???", 0x40, 0, 0,  0, NULL, -2, -2 },
  /* 0x41 */ { "???", 0x41, 0, 0,  0, NULL, -2, -2 },
  /* 0x42 */ { "NOP", 0x42, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0x43 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x44 */ { "NOP", 0x44, 2, 3,  0, (void*) nop, -2, -2 },
  /* 0x45 */ { "???", 0x45, 0, 0,  0, NULL, -2, -2 },
  /* 0x46 */ { "???", 0x46, 0, 0,  0, NULL, -2, -2 },
  /* 0x47 */ { "RMB", 0x47, 2, 5,  0, (void*) rmb4, -2, -2 },
  /* 0x48 */ { "???", 0x48, 0, 0,  0, NULL, -2, -2 },
  /* 0x49 */ { "???", 0x49, 0, 0,  0, NULL, -2, -2 },
  /* 0x4a */ { "???", 0x4a, 0, 0,  0, NULL, -2, -2 },
  /* 0x4b */ { "NOP", 0x4b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x4c */ { "???", 0x4c, 0, 0,  0, NULL, -2, -2 },
  /* 0x4d */ { "???", 0x4d, 0, 0,  0, NULL, -2, -2 },
  /* 0x4e */ { "???", 0x4e, 0, 0,  0, NULL, -2, -2 },
  /* 0x4f */ { "BBR", 0x4f, 3, 5,  0, (void*) bbr4, -2, -2 },

  /* 0x50 */ { "???", 0x50, 0, 0,  0, NULL, -2, -2 },
  /* 0x51 */ { "???", 0x51, 0, 0,  0, NULL, -2, -2 },
  /* 0x52 */ { "EOR", 0x52, 2, 5,  0, (void*) eor_ind, -2, -2 },
  /* 0x53 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x54 */ { "NOP", 0x54, 2, 4,  0, (void*) nop, -2, -2 },
  /* 0x55 */ { "???", 0x55, 0, 0,  0, NULL, -2, -2 },
  /* 0x56 */ { "???", 0x56, 0, 0,  0, NULL, -2, -2 },
  /* 0x57 */ { "RMB", 0x57, 2, 5,  0, (void*) rmb5, -2, -2 },
  /* 0x58 */ { "???", 0x58, 0, 0,  0, NULL, -2, -2 },
  /* 0x59 */ { "???", 0x59, 0, 0,  0, NULL, -2, -2 },
  /* 0x5a */ { "PHY", 0x5a, 1, 3,  0, (void*) phy, -2, -2 },
  /* 0x5b */ { "NOP", 0x5b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x5c */ { "NOP", 0x5c, 3, 8,  0, (void*) nop, -2, -2 },
  /* 0x5d */ { "???", 0x5d, 0, 0,  0, NULL, -2, -2 },
  /* 0x5e */ { "???", 0x5e, 0, 0,  0, NULL, -2, -2 },
  /* 0x5f */ { "BBR", 0x5f, 3, 5,  0, (void*) bbr5, -2, -2 },

  /* 0x60 */ { "???", 0x60, 0, 0,  0, NULL, -2, -2 },
  /* 0x61 */ { "???", 0x61, 0, 0,  0, NULL, -2, -2 },
  /* 0x62 */ { "NOP", 0x62, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0x63 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x64 */ { "STZ", 0x64, 2, 3,  0, (void*) stz_zpg, -2, -2 },
  /* 0x65 */ { "???", 0x65, 0, 0,  0, NULL, -2, -2 },
  /* 0x66 */ { "???", 0x66, 0, 0,  0, NULL, -2, -2 },
  /* 0x67 */ { "RMB", 0x67, 2, 5,  0, (void*) rmb6, -2, -2 },
  /* 0x68 */ { "???", 0x68, 0, 0,  0, NULL, -2, -2 },
  /* 0x69 */ { "???", 0x69, 0, 0,  0, NULL, -2, -2 },
  /* 0x6a */ { "???", 0x6a, 0, 0,  0, NULL, -2, -2 },
  /* 0x6b */ { "NOP", 0x6b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x6c */ { "???", 0x6c, 0, 0,  0, NULL, -2, -2 },
  /* 0x6d */ { "???", 0x6d, 0, 0,  0, NULL, -2, -2 },
  /* 0x6e */ { "???", 0x6e, 0, 0,  0, NULL, -2, -2 },
  /* 0x6f */ { "BBR", 0x6f, 3, 5,  0, (void*) bbr6, -2, -2 },

  /* 0x70 */ { "???", 0x70, 0, 0,  0, NULL, -2, -2 },
  /* 0x71 */ { "???", 0x71, 0, 0,  0, NULL, -2, -2 },
  /* 0x72 */ { "ADC", 0x72, 2, 5,  0, (void*) adc_ind, -2, -2 },
  /* 0x73 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x74 */ { "STZ", 0x74, 2, 4,  0, (void*) stz_zpgx, -2, -2 },
  /* 0x75 */ { "???", 0x75, 0, 0,  0, NULL, -2, -2 },
  /* 0x76 */ { "???", 0x76, 0, 0,  0, NULL, -2, -2 },
  /* 0x77 */ { "RMB", 0x77, 2, 5,  0, (void*) rmb7, -2, -2 },
  /* 0x78 */ { "???", 0x78, 0, 0,  0, NULL, -2, -2 },
  /* 0x79 */ { "???", 0x79, 0, 0,  0, NULL, -2, -2 },
  /* 0x7a */ { "PLY", 0x7a, 1, 4,  0, (void*) ply, -2, -2 },
  /* 0x7b */ { "NOP", 0x7b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x7c */ { "JMP", 0x7c, 3, 6,  0, (void*) jmp_absx, -2, -2 },
  /* 0x7d */ { "???", 0x7d, 0, 0,  0, NULL, -2, -2 },
  /* 0x7e */ { "???", 0x7e, 0, 0,  0, NULL, -2, -2 },
  /* 0x7f */ { "BBR", 0x7f, 3, 5,  0, (void*) bbr7, -2, -2 },

  /* 0x80 */ { "BRA", 0x80, 2, 3,  0, (void*) bra, -2, -2 },
  /* 0x81 */ { "???", 0x81, 0, 0,  0, NULL, -2, -2 },
  /* 0x82 */ { "NOP", 0x82, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0x83 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x84 */ { "???", 0x84, 0, 0,  0, NULL, -2, -2 },
  /* 0x85 */ { "???", 0x85, 0, 0,  0, NULL, -2, -2 },
  /* 0x86 */ { "???", 0x86, 0, 0,  0, NULL, -2, -2 },
  /* 0x87 */ { "SMB", 0x87, 2, 5,  0, (void*) smb0, -2, -2 },
  /* 0x88 */ { "???", 0x88, 0, 0,  0, NULL, -2, -2 },
  /* 0x89 */ { "BIT", 0x89, 2, 2,  0, (void*) bit_imm, -2, -2 },
  /* 0x8a */ { "???", 0x8a, 0, 0,  0, NULL, -2, -2 },
  /* 0x8b */ { "NOP", 0x8b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x8c */ { "???", 0x8c, 0, 0,  0, NULL, -2, -2 },
  /* 0x8d */ { "???", 0x8d, 0, 0,  0, NULL, -2, -2 },
  /* 0x8e */ { "???", 0x8e, 0, 0,  0, NULL, -2, -2 },
  /* 0x8f */ { "BBS", 0x8f, 3, 5,  0, (void*) bbs0, -2, -2 },

  /* 0x90 */ { "???", 0x90, 0, 0,  0, NULL, -2, -2 },
  /* 0x91 */ { "???", 0x91, 0, 0,  0, NULL, -2, -2 },
  /* 0x92 */ { "STA", 0x92, 2, 5,  0, (void*) sta_ind, -2, -2 },
  /* 0x93 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x94 */ { "???", 0x94, 0, 0,  0, NULL, -2, -2 },
  /* 0x95 */ { "???", 0x95, 0, 0,  0, NULL, -2, -2 },
  /* 0x96 */ { "???", 0x96, 0, 0,  0, NULL, -2, -2 },
  /* 0x97 */ { "SMB", 0x97, 2, 5,  0, (void*) smb1, -2, -2 },
  /* 0x98 */ { "???", 0x98, 0, 0,  0, NULL, -2, -2 },
  /* 0x99 */ { "???", 0x99, 0, 0,  0, NULL, -2, -2 },
  /* 0x9a */ { "???", 0x9a, 0, 0,  0, NULL, -2, -2 },
  /* 0x9b */ { "NOP", 0x9b, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0x9c */ { "STZ", 0x9c, 3, 4,  0, (void*) stz_abs, -2, -2 },
  /* 0x9d */ { "???", 0x9d, 0, 0,  0, NULL, -2, -2 },
  /* 0x9e */ { "STZ", 0x9e, 3, 5,  0, (void*) stz_absx, -2, -2 },
  /* 0x9f */ { "BBS", 0x9f, 3, 5,  0, (void*) bbs1, -2, -2 },

  /* 0xa0 */ { "???", 0xa0, 0, 0,  0, NULL, -2, -2 },
  /* 0xa1 */ { "???", 0xa1, 0, 0,  0, NULL, -2, -2 },
  /* 0xa2 */ { "???", 0xa2, 0, 0,  0, NULL, -2, -2 },
  /* 0xa3 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xa4 */ { "???", 0xa4, 0, 0,  0, NULL, -2, -2 },
  /* 0xa5 */ { "???", 0xa5, 0, 0,  0, NULL, -2, -2 },
  /* 0xa6 */ { "???", 0xa6, 0, 0,  0, NULL, -2, -2 },
  /* 0xa7 */ { "SMB", 0xa7, 2, 5,  0, (void*) smb2, -2, -2 },
  /* 0xa8 */ { "???", 0xa8, 0, 0,  0, NULL, -2, -2 },
  /* 0xa9 */ { "???", 0xa9, 0, 0,  0, NULL, -2, -2 },
  /* 0xaa */ { "???", 0xaa, 0, 0,  0, NULL, -2, -2 },
  /* 0xab */ { "NOP", 0xab, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xac */ { "???", 0xac, 0, 0,  0, NULL, -2, -2 },
  /* 0xad */ { "???", 0xad, 0, 0,  0, NULL, -2, -2 },
  /* 0xae */ { "???", 0xae, 0, 0,  0, NULL, -2, -2 },
  /* 0xaf */ { "BBS", 0xaf, 3, 5,  0, (void*) bbs2, -2, -2 },

  /* 0xb0 */ { "???", 0xb0, 0, 0,  0, NULL, -2, -2 },
  /* 0xb1 */ { "???", 0xb1, 0, 0,  0, NULL, -2, -2 },
  /* 0xb2 */ { "LDA", 0xb2, 2, 5,  0, (void*) lda_ind, -2, -2 },
  /* 0xb3 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xb4 */ { "???", 0xb4, 0, 0,  0, NULL, -2, -2 },
  /* 0xb5 */ { "???", 0xb5, 0, 0,  0, NULL, -2, -2 },
  /* 0xb6 */ { "???", 0xb6, 0, 0,  0, NULL, -2, -2 },
  /* 0xb7 */ { "SMB", 0xb7, 2, 5,  0, (void*) smb3, -2, -2 },
  /* 0xb8 */ { "???", 0xb8, 0, 0,  0, NULL, -2, -2 },
  /* 0xb9 */ { "???", 0xb9, 0, 0,  0, NULL, -2, -2 },
  /* 0xba */ { "???", 0xba, 0, 0,  0, NULL, -2, -2 },
  /* 0xbb */ { "NOP", 0xbb, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xbc */ { "???", 0xbc, 0, 0,  0, NULL, -2, -2 },
  /* 0xbd */ { "???", 0xbd, 0, 0,  0, NULL, -2, -2 },
  /* 0xbe */ { "???", 0xbe, 0, 0,  0, NULL, -2, -2 },
  /* 0xbf */ { "BBS", 0xbf, 3, 5,  0, (void*) bbs3, -2, -2 },

  /* 0xc0 */ { "???", 0xc0, 0, 0,  0, NULL, -2, -2 },
  /* 0xc1 */ { "???", 0xc1, 0, 0,  0, NULL, -2, -2 },
  /* 0xc2 */ { "NOP", 0xc2, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0xc3 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xc4 */ { "???", 0xc4, 0, 0,  0, NULL, -2, -2 },
  /* 0xc5 */ { "???", 0xc5, 0, 0,  0, NULL, -2, -2 },
  /* 0xc6 */ { "???", 0xc6, 0, 0,  0, NULL, -2, -2 },
  /* 0xc7 */ { "SMB", 0xc7, 2, 5,  0, (void*) smb4, -2, -2 },
  /* 0xc8 */ { "???", 0xc8, 0, 0,  0, NULL, -2, -2 },
  /* 0xc9 */ { "???", 0xc9, 0, 0,  0, NULL, -2, -2 },
  /* 0xca */ { "???", 0xca, 0, 0,  0, NULL, -2, -2 },
  /* 0xcb */ { "NOP", 0xcb, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xcc */ { "???", 0xcc, 0, 0,  0, NULL, -2, -2 },
  /* 0xcd */ { "???", 0xcd, 0, 0,  0, NULL, -2, -2 },
  /* 0xce */ { "???", 0xce, 0, 0,  0, NULL, -2, -2 },
  /* 0xcf */ { "BBS", 0xcf, 3, 5,  0, (void*) bbs4, -2, -2 },

  /* 0xd0 */ { "???", 0xd0, 0, 0,  0, NULL, -2, -2 },
  /* 0xd1 */ { "???", 0xd1, 0, 0,  0, NULL, -2, -2 },
  /* 0xd2 */ { "CMP", 0xd2, 2, 5,  0, (void*) cmp_ind, -2, -2 },
  /* 0xd3 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xd4 */ { "NOP", 0xd4, 2, 4,  0, (void*) nop, -2, -2 },
  /* 0xd5 */ { "???", 0xd5, 0, 0,  0, NULL, -2, -2 },
  /* 0xd6 */ { "???", 0xd6, 0, 0,  0, NULL, -2, -2 },
  /* 0xd7 */ { "SMB", 0xd7, 2, 5,  0, (void*) smb5, -2, -2 },
  /* 0xd8 */ { "???", 0xd8, 0, 0,  0, NULL, -2, -2 },
  /* 0xd9 */ { "???", 0xd9, 0, 0,  0, NULL, -2, -2 },
  /* 0xda */ { "PHX", 0xda, 1, 3,  0, (void*) phx, -2, -2 },
  /* 0xdb */ { "NOP", 0xdb, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xdc */ { "NOP", 0xdc, 3, 4,  0, (void*) nop, -2, -2 },
  /* 0xdd */ { "???", 0xdd, 0, 0,  0, NULL, -2, -2 },
  /* 0xde */ { "???", 0xde, 0, 0,  0, NULL, -2, -2 },
  /* 0xdf */ { "BBS", 0xdf, 3, 5,  0, (void*) bbs5, -2, -2 },

  /* 0xe0 */ { "???", 0xe0, 0, 0,  0, NULL, -2, -2 },
  /* 0xe1 */ { "???", 0xe1, 0, 0,  0, NULL, -2, -2 },
  /* 0xe2 */ { "NOP", 0xe2, 2, 2,  0, (void*) nop, -2, -2 },
  /* 0xe3 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xe4 */ { "???", 0xe4, 0, 0,  0, NULL, -2, -2 },
  /* 0xe5 */ { "???", 0xe5, 0, 0,  0, NULL, -2, -2 },
  /* 0xe6 */ { "???", 0xe6, 0, 0,  0, NULL, -2, -2 },
  /* 0xe7 */ { "SMB", 0xe7, 2, 5,  0, (void*) smb6, -2, -2 },
  /* 0xe8 */ { "???", 0xe8, 0, 0,  0, NULL, -2, -2 },
  /* 0xe9 */ { "???", 0xe9, 0, 0,  0, NULL, -2, -2 },
  /* 0xea */ { "???", 0xea, 0, 0,  0, NULL, -2, -2 },
  /* 0xeb */ { "NOP", 0xeb, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xec */ { "???", 0xec, 0, 0,  0, NULL, -2, -2 },
  /* 0xed */ { "???", 0xed, 0, 0,  0, NULL, -2, -2 },
  /* 0xee */ { "???", 0xee, 0, 0,  0, NULL, -2, -2 },
  /* 0xef */ { "BBS", 0xef, 3, 5,  0, (void*) bbs6, -2, -2 },

  /* 0xf0 */ { "???", 0xf0, 0, 0,  0, NULL, -2, -2 },
  /* 0xf1 */ { "???", 0xf1, 0, 0,  0, NULL, -2, -2 },
  /* 0xf2 */ { "SBC", 0xf2, 2, 5,  0, (void*) sbc_ind, -2, -2 },
  /* 0xf3 */ { "NOP", 0x03, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xf4 */ { "NOP", 0xf4, 2, 4,  0, (void*) nop, -2, -2 },
  /* 0xf5 */ { "???", 0xf5, 0, 0,  0, NULL, -2, -2 },
  /* 0xf6 */ { "???", 0xf6, 0, 0,  0, NULL, -2, -2 },
  /* 0xf7 */ { "SMB", 0xf7, 2, 5,  0, (void*) smb7, -2, -2 },
  /* 0xf8 */ { "???", 0xf8, 0, 0,  0, NULL, -2, -2 },
  /* 0xf9 */ { "???", 0xf9, 0, 0,  0, NULL, -2, -2 },
  /* 0xfa */ { "PLX", 0xfa, 1, 4,  0, (void*) plx, -2, -2 },
  /* 0xfb */ { "NOP", 0xfb, 1, 1,  0, (void*) nop, -2, -2 },
  /* 0xfc */ { "NOP", 0xfc, 3, 4,  0, (void*) nop, -2, -2 },
  /* 0xfd */ { "???", 0xfd, 0, 0,  0, NULL, -2, -2 },
  /* 0xfe */ { "???", 0xfe, 0, 0,  0, NULL, -2, -2 },
  /* 0xff */ { "BBS", 0xff, 3, 5,  0, (void*) bbs7, -2, -2 }
};
