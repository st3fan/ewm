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

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

#define EWM_CPU_MODEL_6502  0
#define EWM_CPU_MODEL_65C02 1

#define EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION (-1)
#define EWM_CPU_ERR_STACK_OVERFLOW            (-2)
#define EWM_CPU_ERR_STACK_UNDERFLOW           (-3)

#define EWM_VECTOR_NMI 0xfffa
#define EWM_VECTOR_RES 0xfffc
#define EWM_VECTOR_IRQ 0xfffe

struct cpu_instruction_t;

struct cpu_state_t {
  uint8_t a, x, y, s, sp;
  uint16_t pc;
  uint8_t n, v, b, d, i, z, c;
};

struct cpu_t {
   int model;
   struct cpu_state_t state;
   FILE *trace;
   bool strict;
   struct mem_t *mem;
   uint8_t *memory; // This is pointing to the first 2 pages of memory, zero page and stack.
   struct cpu_instruction_t *instructions;
};

#define MEM_TYPE_RAM 0
#define MEM_TYPE_ROM 1
#define MEM_TYPE_IOM 2

typedef uint8_t (*mem_read_handler_t)(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr);
typedef void (*mem_write_handler_t)(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b);

struct mem_t {
  uint8_t type;
  uint16_t start;
  uint16_t end;
  void *obj;
  mem_read_handler_t read_handler;
  mem_write_handler_t write_handler;
  struct mem_t *next;
};

// Private. How do we keep them private?
void _cpu_push_byte(struct cpu_t *cpu, uint8_t b);
void _cpu_push_word(struct cpu_t *cpu, uint16_t w);
uint8_t _cpu_pull_byte(struct cpu_t *cpu);
uint16_t _cpu_pull_word(struct cpu_t *cpu);

uint8_t _cpu_stack_free(struct cpu_t *cpu);
uint8_t _cpu_stack_used(struct cpu_t *cpu);

// Private. How do we keep them private?
uint8_t _cpu_get_status(struct cpu_t *cpu);
void _cpu_set_status(struct cpu_t *cpu, uint8_t status);

void cpu_setup();

void cpu_init(struct cpu_t *cpu, int model);
void cpu_shutdown(struct cpu_t *cpu);

struct mem_t *cpu_add_mem(struct cpu_t *cpu, struct mem_t *mem);
struct mem_t *cpu_add_ram(struct cpu_t *cpu, uint16_t start, uint16_t end);
struct mem_t *cpu_add_ram_data(struct cpu_t *cpu, uint16_t start, uint16_t end, uint8_t *data);
struct mem_t *cpu_add_ram_file(struct cpu_t *cpu, uint16_t start, char *path);
struct mem_t *cpu_add_rom_data(struct cpu_t *cpu, uint16_t start, uint16_t end, uint8_t *data);
struct mem_t *cpu_add_rom_file(struct cpu_t *cpu, uint16_t start, char *path);
struct mem_t *cpu_add_iom(struct cpu_t *cpu, uint16_t start, uint16_t end, void *obj, mem_read_handler_t read_handler, mem_write_handler_t write_handler);

void cpu_strict(struct cpu_t *cpu, bool strict);
int cpu_trace(struct cpu_t *cpu, char *path);

void cpu_reset(struct cpu_t *cpu);
int cpu_irq(struct cpu_t *cpu);
int cpu_nmi(struct cpu_t *cpu);

int cpu_run(struct cpu_t *cpu);
int cpu_boot(struct cpu_t *cpu);
int cpu_step(struct cpu_t *cpu);

uint16_t cpu_memory_get_word(struct cpu_t *cpu, uint16_t addr);
uint8_t cpu_memory_get_byte(struct cpu_t *cpu, uint16_t addr);

void cpu_memory_set_word(struct cpu_t *cpu, uint16_t addr, uint16_t v);
void cpu_memory_set_byte(struct cpu_t *cpu, uint16_t addr, uint8_t v);

#endif
