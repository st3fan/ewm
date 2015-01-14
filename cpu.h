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

#ifndef CPU_H
#define CPU_H

#include <stdint.h>

struct iom_t {
  uint16_t start;
  uint16_t length;
  void *obj;
  void *read_handler;
  void *write_handler;
  struct iom_t *next;
};

struct cpu_state_t {
  uint8_t a, x, y, s;
  uint16_t sp;
  uint16_t pc;
  uint8_t n, v, b, d, i, z, c;
};

struct cpu_t {
  struct cpu_state_t state;
  uint8_t *memory;
  uint8_t trace;
  struct iom_t *iom;
};

typedef uint8_t (*iom_read_handler_t)(struct cpu_t *cpu, void *obj, uint16_t addr);
typedef void (*iom_write_handler_t)(struct cpu_t *cpu, void *obj, uint16_t addr, uint8_t b);

void cpu_init(struct cpu_t *cpu);
void cpu_add_ram(struct cpu_t *cpu, uint16_t start, uint16_t length, uint8_t *data);
void cpu_add_rom(struct cpu_t *cpu, uint16_t start, uint16_t length, uint8_t *data);
void cpu_add_iom(struct cpu_t *cpu, uint16_t start, uint16_t length, void *obj, iom_read_handler_t read_handler, iom_write_handler_t write_handler);

void cpu_trace(struct cpu_t *cpu, uint8_t trace);

void cpu_reset(struct cpu_t *cpu);
void cpu_brk(struct cpu_t *cpu);
void cpu_irq(struct cpu_t *cpu);
void cpu_nmi(struct cpu_t *cpu);

void cpu_run(struct cpu_t *cpu);
void cpu_boot(struct cpu_t *cpu);
void cpu_step(struct cpu_t *cpu);

uint16_t cpu_memory_get_word(struct cpu_t *cpu, uint16_t addr);
uint8_t cpu_memory_get_byte(struct cpu_t *cpu, uint16_t addr);

void cpu_memory_set_word(struct cpu_t *cpu, uint16_t addr, uint16_t v);
void cpu_memory_set_byte(struct cpu_t *cpu, uint16_t addr, uint8_t v);

#endif
