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

#ifndef MEM_H
#define MEM_H

#include <stdint.h>

struct cpu_t;

typedef uint8_t (*mem_mod_t)(struct cpu_t *cpu, uint8_t b);

uint8_t mem_get_byte(struct cpu_t *cpu, uint16_t addr);
void mem_set_byte(struct cpu_t *cpu, uint16_t addr, uint8_t v);

uint8_t mem_get_byte_abs(struct cpu_t *cpu, uint16_t addr);
uint8_t mem_get_byte_absx(struct cpu_t *cpu, uint16_t addr);
uint8_t mem_get_byte_absy(struct cpu_t *cpu, uint16_t addr);
uint8_t mem_get_byte_zpg(struct cpu_t *cpu, uint8_t addr);
uint8_t mem_get_byte_zpgx(struct cpu_t *cpu, uint8_t addr);
uint8_t mem_get_byte_zpgy(struct cpu_t *cpu, uint8_t addr);
uint8_t mem_get_byte_indx(struct cpu_t *cpu, uint8_t addr);
uint8_t mem_get_byte_indy(struct cpu_t *cpu, uint8_t addr);
uint8_t mem_get_byte_ind(struct cpu_t *cpu, uint8_t addr);

void mem_set_byte_zpg(struct cpu_t *cpu, uint8_t addr, uint8_t v);
void mem_set_byte_zpgx(struct cpu_t *cpu, uint8_t addr, uint8_t v);
void mem_set_byte_zpgy(struct cpu_t *cpu, uint8_t addr, uint8_t v);
void mem_set_byte_abs(struct cpu_t *cpu, uint16_t addr, uint8_t v);
void mem_set_byte_absx(struct cpu_t *cpu, uint16_t addr, uint8_t v);
void mem_set_byte_absy(struct cpu_t *cpu, uint16_t addr, uint8_t v);
void mem_set_byte_indx(struct cpu_t *cpu, uint8_t addr, uint8_t v);
void mem_set_byte_indy(struct cpu_t *cpu, uint8_t addr, uint8_t v);
void mem_set_byte_ind(struct cpu_t *cpu, uint8_t addr, uint8_t v);

void mem_mod_byte_zpg(struct cpu_t *cpu, uint8_t addr, mem_mod_t op);
void mem_mod_byte_zpgx(struct cpu_t *cpu, uint8_t addr, mem_mod_t op);
void mem_mod_byte_zpgy(struct cpu_t *cpu, uint8_t addr, mem_mod_t op);
void mem_mod_byte_abs(struct cpu_t *cpu, uint16_t addr, mem_mod_t op);
void mem_mod_byte_absx(struct cpu_t *cpu, uint16_t addr, mem_mod_t op);
void mem_mod_byte_absy(struct cpu_t *cpu, uint16_t addr, mem_mod_t op);
void mem_mod_byte_indx(struct cpu_t *cpu, uint8_t addr, mem_mod_t op);
void mem_mod_byte_indy(struct cpu_t *cpu, uint8_t addr, mem_mod_t op);

uint16_t mem_get_word(struct cpu_t *cpu, uint16_t addr);
void mem_set_word(struct cpu_t *cpu, uint16_t addr, uint16_t v);

// For parsing --memory options

#define EWM_MEMORY_TYPE_RAM (0)
#define EWM_MEMORY_TYPE_ROM (1)

struct ewm_memory_option_t {
   int type;
   char *path;
   uint16_t address;
   struct ewm_memory_option_t *next;
};

struct ewm_memory_option_t *parse_memory_option(char *s);
int cpu_add_memory_from_options(struct cpu_t *cpu, struct ewm_memory_option_t *m);

#endif
