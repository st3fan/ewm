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

#include "mem.h"

/* Memory Access */

uint8_t mem_get_byte(struct cpu_t *cpu, uint16_t addr) {
  if (cpu->iom) {
    /* TODO: Assuming there is only one area of memory mapped io */
    if (addr >= cpu->iom->start && addr < (cpu->iom->start + cpu->iom->length)) {
      uint8_t v = ((iom_read_handler_t) cpu->iom->read_handler)((struct cpu_t*) cpu, cpu->iom->obj, addr);
      //fprintf(stderr, "MEM: GETTING BYTE AT %.4X -> %.2X\n", addr, v);
      return v;
    }
  }
  //fprintf(stderr, "MEM: GETTING BYTE AT %.4X -> %.2X\n", addr, cpu->memory[addr]);
  return cpu->memory[addr];
}

uint8_t mem_get_byte_abs(struct cpu_t *cpu, uint16_t addr) {
  return mem_get_byte(cpu, addr);
}

uint8_t mem_get_byte_absx(struct cpu_t *cpu, uint16_t addr) {
  return mem_get_byte(cpu, addr + cpu->state.x); /* TODO: Carry? */
}

uint8_t mem_get_byte_absy(struct cpu_t *cpu, uint16_t addr) {
  return mem_get_byte(cpu, addr + cpu->state.y); /* TODO: Carry? */
}

uint8_t mem_get_byte_zpg(struct cpu_t *cpu, uint8_t addr) {
  return mem_get_byte(cpu, addr);
}

uint8_t mem_get_byte_zpgx(struct cpu_t *cpu, uint8_t addr) {
  return mem_get_byte(cpu, ((uint16_t) addr + cpu->state.x) & 0x00ff);
}

uint8_t mem_get_byte_zpgy(struct cpu_t *cpu, uint8_t addr) {
  return mem_get_byte(cpu, ((uint16_t) addr + cpu->state.y) & 0x00ff);
}

uint8_t mem_get_byte_indx(struct cpu_t *cpu, uint8_t addr) {
  return mem_get_byte(cpu, mem_get_word(cpu, addr + cpu->state.x)); // TODO: Does this wrap?
}

uint8_t mem_get_byte_indy(struct cpu_t *cpu, uint8_t addr) {
  return mem_get_byte(cpu, mem_get_word(cpu, addr) + cpu->state.y);
}



void mem_set_byte(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
  //fprintf(stderr, "MEM: SETTING BYTE AT %.4X -> %.2X (%c)\n", addr, v, v & 0x7f);
  if (cpu->iom) {
    /* TODO: Assuming there is only one area of memory mapped io */
    if (addr >= cpu->iom->start && addr < (cpu->iom->start + cpu->iom->length)) {
      ((iom_write_handler_t) cpu->iom->write_handler)((struct cpu_t*) cpu, cpu->iom->obj, addr, v);
      return;
    }
  }
  cpu->memory[addr] = v;
}



void mem_set_byte_zpg(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
  mem_set_byte(cpu, addr, v);
}

void mem_set_byte_zpgx(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
  mem_set_byte(cpu, ((uint16_t) addr + cpu->state.x) & 0x00ff, v);
}

void mem_set_byte_zpgy(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
  mem_set_byte(cpu, ((uint16_t) addr + cpu->state.y) & 0x00ff, v);
}

void mem_set_byte_abs(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
  mem_set_byte(cpu, addr, v);
}

void mem_set_byte_absx(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
  mem_set_byte(cpu, addr+cpu->state.x, v);
}

void mem_set_byte_absy(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
  mem_set_byte(cpu, addr+cpu->state.y, v);
}

void mem_set_byte_indx(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
  mem_set_byte(cpu, mem_get_word(cpu, addr+cpu->state.x), v); // TODO: Does this wrap?
}

void mem_set_byte_indy(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
  mem_set_byte(cpu, mem_get_word(cpu, addr)+cpu->state.y, v);
}

/* MOD */

void mem_mod_byte_zpg(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_zpg(cpu, addr, op(cpu, mem_get_byte_zpg(cpu, addr)));
}

void mem_mod_byte_zpgx(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_zpgx(cpu, addr, op(cpu, mem_get_byte_zpgx(cpu, addr)));
}

void mem_mod_byte_zpgy(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_zpgy(cpu, addr, op(cpu, mem_get_byte_zpgy(cpu, addr)));
}

void mem_mod_byte_abs(struct cpu_t *cpu, uint16_t addr, mem_mod_t op) {
  mem_set_byte_abs(cpu, addr, op(cpu, mem_get_byte_abs(cpu, addr)));
}

void mem_mod_byte_absx(struct cpu_t *cpu, uint16_t addr, mem_mod_t op) {
  mem_set_byte_absx(cpu, addr, op(cpu, mem_get_byte_absx(cpu, addr)));
}

void mem_mod_byte_absy(struct cpu_t *cpu, uint16_t addr, mem_mod_t op) {
  mem_set_byte_absy(cpu, addr, op(cpu, mem_get_byte_absy(cpu, addr)));
}

void mem_mod_byte_indx(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_indx(cpu, addr, op(cpu, mem_get_byte_indx(cpu, addr)));
}

void mem_mod_byte_indy(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_indy(cpu, addr, op(cpu, mem_get_byte_indy(cpu, addr)));
}

/* Words */

uint16_t mem_get_word(struct cpu_t *cpu, uint16_t addr) {
  return *((uint16_t*) &cpu->memory[addr]);
}

void mem_set_word(struct cpu_t *cpu, uint16_t addr, uint16_t v) {
   *((uint16_t*) &cpu->memory[addr]) = v;
}
