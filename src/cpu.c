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

#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <inttypes.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/stat.h>

#include "cpu.h"
#include "ins.h"
#include "mem.h"
#include "fmt.h"

/* Private API */

typedef void (*cpu_instruction_handler_t)(struct cpu_t *cpu);
typedef void (*cpu_instruction_handler_byte_t)(struct cpu_t *cpu, uint8_t oper);
typedef void (*cpu_instruction_handler_word_t)(struct cpu_t *cpu, uint16_t oper);

// Stack management.

void _cpu_push_byte(struct cpu_t *cpu, uint8_t b) {
   mem_set_byte(cpu, 0x0100 + cpu->state.sp, b);
  cpu->state.sp -= 1;
}

void _cpu_push_word(struct cpu_t *cpu, uint16_t w) {
  _cpu_push_byte(cpu, (uint8_t) (w >> 8));
  _cpu_push_byte(cpu, (uint8_t) w);
}

uint8_t _cpu_pull_byte(struct cpu_t *cpu) {
  cpu->state.sp += 1;
  return mem_get_byte(cpu, 0x0100 + cpu->state.sp);
}

uint16_t _cpu_pull_word(struct cpu_t *cpu) {
  return (uint16_t) _cpu_pull_byte(cpu) | ((uint16_t) _cpu_pull_byte(cpu) << 8);
}

uint8_t _cpu_stack_free(struct cpu_t *cpu) {
   return cpu->state.sp;
}

uint8_t _cpu_stack_used(struct cpu_t *cpu) {
   return 0xff - cpu->state.sp;
}

// Because we keep the processor status bits in separate fields, we
// need a function to combine them into a single register. This is
// only used when we need to push the register on the stack for
// interupt handlers. If this turns out to be inefficient then they
// can be stored in their native form in a byte.

uint8_t _cpu_get_status(struct cpu_t *cpu) {
  return 0x30
    | (((cpu->state.n != 0) & 0x01) << 7)
    | (((cpu->state.v != 0) & 0x01) << 6)
    | (((cpu->state.b != 0) & 0x01) << 4)
    | (((cpu->state.d != 0) & 0x01) << 3)
    | (((cpu->state.i != 0) & 0x01) << 2)
    | (((cpu->state.z != 0) & 0x01) << 1)
    | (((cpu->state.c != 0) & 0x01) << 0);
}

void _cpu_set_status(struct cpu_t *cpu, uint8_t status) {
  cpu->state.n = (status & (1 << 7));
  cpu->state.v = (status & (1 << 6));
  cpu->state.b = (status & (1 << 4));
  cpu->state.d = (status & (1 << 3));
  cpu->state.i = (status & (1 << 2));
  cpu->state.z = (status & (1 << 1));
  cpu->state.c = (status & (1 << 0));
}

static int cpu_execute_instruction(struct cpu_t *cpu) {
   /* Trace code - Refactor into its own function or module */
   char trace_instruction[256];
   char trace_state[256];
   char trace_stack[256];

   if (cpu->trace) {
      cpu_format_instruction(cpu, trace_instruction);
   }

   /* Fetch instruction */
   struct cpu_instruction_t *i = &cpu->instructions[mem_get_byte(cpu, cpu->state.pc)];
   if (i->name[0] == '?') {
      if (cpu->strict) {
         return EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION;
      }
   }

   // If strict mode and if we need the stack, check if that works out
   if (cpu->strict && i->stack != 0) {
      if (i->stack > 0) {
         if (_cpu_stack_free(cpu) < i->stack) {
            return EWM_CPU_ERR_STACK_OVERFLOW;
         }
      } else {
         if (_cpu_stack_used(cpu) < -(i->stack)) {
            return EWM_CPU_ERR_STACK_UNDERFLOW;
         }
      }
   }

   /* Remember the PC since some instructions modify it */
   uint16_t pc = cpu->state.pc;

   /* Advance PC */
   if (pc == cpu->state.pc) {
      cpu->state.pc += i->bytes;
   }

   /* Execute instruction */
   switch (i->bytes) {
      case 1:
         ((cpu_instruction_handler_t) i->handler)(cpu);
         break;
      case 2:
         ((cpu_instruction_handler_byte_t) i->handler)(cpu, mem_get_byte(cpu, pc+1));
         break;
      case 3:
         ((cpu_instruction_handler_word_t) i->handler)(cpu, mem_get_word(cpu, pc+1));
         break;
   }

   if (cpu->trace) {
      cpu_format_state(cpu, trace_state);
      cpu_format_stack(cpu, trace_stack);

      char bytes[10];
      switch (i->bytes) {
         case 1:
            snprintf(bytes, sizeof bytes, "%.2X", mem_get_byte(cpu, pc));
            break;
         case 2:
            snprintf(bytes, sizeof bytes, "%.2X %.2X", mem_get_byte(cpu, pc), mem_get_byte(cpu, pc+1));
            break;
         case 3:
            snprintf(bytes, sizeof bytes, "%.2X %.2X %.2X", mem_get_byte(cpu, pc), mem_get_byte(cpu, pc+1), mem_get_byte(cpu, pc+2));
            break;
      }

      fprintf(cpu->trace, "%.4X: %-8s  %-14s  %-20s  %s\n",
              pc, bytes, trace_instruction, trace_state, trace_stack);
   }

   cpu->counter += i->cycles;

   return i->cycles;
}

/* Public API */

static bool cpu_initialized = false;

static void cpu_initialize() {
   for (int i = 0; i <= 255; i++) {
      if (instructions_65C02[i].handler == NULL) {
         instructions_65C02[i] = instructions[i];
      }
   }
}

int cpu_init(struct cpu_t *cpu, int model) {
   if (!cpu_initialized) {
      cpu_initialize();
      cpu_initialized = true;
   }

   memset(cpu, 0x00, sizeof(struct cpu_t));
   cpu->model = model;
   cpu->instructions = (cpu->model == EWM_CPU_MODEL_6502) ? instructions : instructions_65C02;

   return 0;
}

struct cpu_t *cpu_create(int model) {
   struct cpu_t *cpu = malloc(sizeof(struct cpu_t));
   if (cpu_init(cpu, model) != 0) {
      free(cpu);
      cpu = NULL;
   }
   return cpu;
}

void cpu_destroy(struct cpu_t *cpu) {
   if (cpu->trace != NULL) {
      (void) fclose(cpu->trace);
      cpu->trace = NULL;
   }
}

struct mem_t *cpu_add_mem(struct cpu_t *cpu, struct mem_t *mem) {
  if (cpu->mem == NULL) {
    cpu->mem = mem;
    mem->next = NULL;
  } else {
    mem->next = cpu->mem;
    cpu->mem = mem;
  }
  return mem;
}

// RAM Memory

static uint8_t _ram_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return ((uint8_t*) mem->obj)[addr - mem->start];
}

static void _ram_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
  ((uint8_t*) mem->obj)[addr - mem->start] = b;
}

struct mem_t *cpu_add_ram(struct cpu_t *cpu, uint16_t start, uint16_t end) {
   return cpu_add_ram_data(cpu, start, end, calloc(end-start+1, 0x01));
}

struct mem_t *cpu_add_ram_data(struct cpu_t *cpu, uint16_t start, uint16_t end, uint8_t *data) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  memset(mem, 0, sizeof(struct mem_t));
  mem->enabled = true;
  mem->flags = MEM_FLAGS_READ | MEM_FLAGS_WRITE;
  mem->obj = data;
  mem->start = start;
  mem->end = end;
  mem->read_handler = _ram_read;
  mem->write_handler = _ram_write;
  mem->next = NULL;
  return cpu_add_mem(cpu, mem);
}

struct mem_t *cpu_add_ram_file(struct cpu_t *cpu, uint16_t start, char *path) {
   int fd = open(path, O_RDONLY);
   if (fd == -1) {
      return NULL;
   }

   struct stat file_info;
   if (fstat(fd, &file_info) == -1) {
      close(fd);
      return NULL;
   }

   if (file_info.st_size  > (64 * 1024 - start)) {
      close(fd);
      return NULL;
   }

   char *data = calloc(file_info.st_size, 1);
   if (read(fd, data, file_info.st_size) != file_info.st_size) {
      close(fd);
      return NULL;
   }

   close(fd);

   return cpu_add_ram_data(cpu, start, start + file_info.st_size - 1, (uint8_t*) data);
}

// ROM Memory

static uint8_t _rom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return ((uint8_t*) mem->obj)[addr - mem->start];
}

struct mem_t *cpu_add_rom_data(struct cpu_t *cpu, uint16_t start, uint16_t end, uint8_t *data) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  memset(mem, 0, sizeof(struct mem_t));
  mem->enabled = true;
  mem->flags = MEM_FLAGS_READ;
  mem->obj = data;
  mem->start = start;
  mem->end = end;
  mem->read_handler = _rom_read;
  mem->write_handler = NULL;
  mem->next = NULL;
  return cpu_add_mem(cpu, mem);
}

struct mem_t *cpu_add_rom_file(struct cpu_t *cpu, uint16_t start, char *path) {
   int fd = open(path, O_RDONLY);
   if (fd == -1) {
      return NULL;
   }

   struct stat file_info;
   if (fstat(fd, &file_info) == -1) {
      close(fd);
      return NULL;
   }

   if (file_info.st_size  > (64 * 1024 - start)) {
      close(fd);
      return NULL;
   }

   char *data = calloc(file_info.st_size, 1);
   if (read(fd, data, file_info.st_size) != file_info.st_size) {
      close(fd);
      return NULL;
   }

   close(fd);

   struct mem_t *result = cpu_add_rom_data(cpu, start, start + file_info.st_size - 1, (uint8_t*) data);
   result->description = path;
   return result;
}

// IO Memory

struct mem_t *cpu_add_iom(struct cpu_t *cpu, uint16_t start, uint16_t end, void *obj, mem_read_handler_t read_handler, mem_write_handler_t write_handler) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  memset(mem, 0, sizeof(struct mem_t));
  mem->enabled = true;
  mem->flags = MEM_FLAGS_READ | MEM_FLAGS_WRITE;
  mem->obj = obj;
  mem->start = start;
  mem->end = end;
  mem->read_handler = read_handler;
  mem->write_handler = write_handler;
  mem->next = NULL;
  return cpu_add_mem(cpu, mem);
}

void cpu_strict(struct cpu_t *cpu, bool strict) {
   cpu->strict = strict;
}

int cpu_trace(struct cpu_t *cpu, char *path) {
   if (cpu->trace != NULL) {
      (void) fclose(cpu->trace);
      cpu->trace = NULL;
   }

   if (path != NULL) {
      cpu->trace = fopen(path, "w");
      if (cpu->trace == NULL) {
         return errno;
      }
   }

   return 0;
}

void cpu_reset(struct cpu_t *cpu) {
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_RES);
   cpu->state.a = 0x00;
   cpu->state.x = 0x00;
   cpu->state.y = 0x00;
   cpu->state.n = 0;
   cpu->state.v = 0;
   cpu->state.b = 0;
   cpu->state.d = 0;
   cpu->state.i = 1;
   cpu->state.z = 0;
   cpu->state.c = 0;
   cpu->state.sp = 0xff;
}

int cpu_irq(struct cpu_t *cpu) {
   if (cpu->strict && _cpu_stack_free(cpu) < 3) {
      return EWM_CPU_ERR_STACK_OVERFLOW;
   }

   _cpu_push_word(cpu, cpu->state.pc + 1); // TODO +1?? Spec says +2 but test fails then
   _cpu_push_byte(cpu, _cpu_get_status(cpu));
   cpu->state.i = 1;
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_IRQ);

   return 0;
}

int cpu_nmi(struct cpu_t *cpu) {
   if (cpu->strict && _cpu_stack_free(cpu) < 3) {
      return EWM_CPU_ERR_STACK_OVERFLOW;
   }

   _cpu_push_word(cpu, cpu->state.pc + 1); // TODO +1?? Spec says +2 but test fails then
   _cpu_push_byte(cpu, _cpu_get_status(cpu));
   cpu->state.i = 1;
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_NMI);

   return 0;
}

int cpu_step(struct cpu_t *cpu) {
   return cpu_execute_instruction(cpu);
}
