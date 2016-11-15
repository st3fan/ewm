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
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <inttypes.h>
#include <stdlib.h>
#include <unistd.h>

#include <mach/mach.h>
#include <mach/mach_time.h>

#include "cpu.h"
#include "ins.h"
#include "mem.h"

/* Private API */

typedef void (*cpu_instruction_handler_t)(struct cpu_t *cpu);
typedef void (*cpu_instruction_handler_byte_t)(struct cpu_t *cpu, uint8_t oper);
typedef void (*cpu_instruction_handler_word_t)(struct cpu_t *cpu, uint16_t oper);

// Stack management.

void _cpu_push_byte(struct cpu_t *cpu, uint8_t b) {
  _mem_set_byte_direct(cpu, 0x0100 + cpu->state.sp, b);
  cpu->state.sp -= 1;
}

void _cpu_push_word(struct cpu_t *cpu, uint16_t w) {
  _cpu_push_byte(cpu, (uint8_t) (w >> 8));
  _cpu_push_byte(cpu, (uint8_t) w);
}

uint8_t _cpu_pull_byte(struct cpu_t *cpu) {
  cpu->state.sp += 1;
  return _mem_get_byte_direct(cpu, 0x0100 + cpu->state.sp);
}

uint16_t _cpu_pull_word(struct cpu_t *cpu) {
  return (uint16_t) _cpu_pull_byte(cpu) | ((uint16_t) _cpu_pull_byte(cpu) << 8);
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

static void cpu_format_instruction(struct cpu_t *cpu, char *buffer) {
   *buffer = 0x00;

   cpu_instruction_t *i = &instructions[mem_get_byte(cpu, cpu->state.pc)];
   uint8_t opcode = mem_get_byte(cpu, cpu->state.pc);

   /* Single byte instructions */
   if (i->bytes == 1) {
      sprintf(buffer, "%s", i->name);
   }

   /* JSR is the only exception */
   else if (opcode == 0x20) {
     sprintf(buffer, "%s $%.4X", i->name, mem_get_word(cpu, cpu->state.pc+1));
   }

   /* Branches */
   else if ((opcode & 0b00011111) == 0b00010000) {
      int8_t offset = (int8_t) mem_get_byte(cpu, cpu->state.pc+1);
      uint16_t addr = cpu->state.pc + 2 + offset;
      sprintf(buffer, "%s $%.4X", i->name, addr);
   }

   else if ((opcode & 0b00000011) == 0b00000001) {
      switch ((opcode & 0b00011100) >> 2) {
         case 0b000:
            sprintf(buffer, "%s ($%.2X,X)", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b001:
            sprintf(buffer, "%s $%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b010:
            sprintf(buffer, "%s #$%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b011:
            sprintf(buffer, "%s $%.2X%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b100:
            sprintf(buffer, "%s ($%.2X),Y", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b101:
            sprintf(buffer, "%s $%.2X,X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b110:
            sprintf(buffer, "%s $%.2X%.2X,Y", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b111:
            sprintf(buffer, "%s $%.2X%.2X,X", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
      }
   }

   else if ((opcode & 0b00000011) == 0b00000010) {
      switch ((opcode & 0b00011100) >> 2) {
         case 0b000:
            sprintf(buffer, "%s #$%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b001:
            sprintf(buffer, "%s $%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b010:
            sprintf(buffer, "%s", i->name);
            break;
         case 0b011:
            sprintf(buffer, "%s $%.2X%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b101:
            sprintf(buffer, "%s $%.2X,X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b111:
            sprintf(buffer, "%s $%.2X%.2X,X", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
      }
   }

   else if ((opcode & 0b00000011) == 0b00000000) {
      switch ((opcode & 0b00011100) >> 2) {
         case 0b000:
            sprintf(buffer, "%s #$%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b001:
            sprintf(buffer, "%s $%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b011:
            sprintf(buffer, "%s $%.2X%.2X", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b101:
            sprintf(buffer, "%s $%.2X,X", i->name, mem_get_byte(cpu, cpu->state.pc+1));
            break;
         case 0b111:
            sprintf(buffer, "%s $%.2X%.2X,X", i->name, mem_get_byte(cpu, cpu->state.pc+2), mem_get_byte(cpu, cpu->state.pc+1));
            break;
      }
   }
}

static void cpu_format_state(struct cpu_t *cpu, char *buffer) {
   sprintf(buffer, "A=%.2X X=%.2X Y=%.2X S=%.2X SP=%.4X %c%c%c%c%c%c%c%c",
           cpu->state.a, cpu->state.x, cpu->state.y, cpu->state.s, 0x0100 + cpu->state.sp,

           cpu->state.n ? 'N' : '-',
           cpu->state.v ? 'V' : '-',
           '-',
           cpu->state.b ? 'B' : '-',
           cpu->state.d ? 'D' : '-',
           cpu->state.i ? 'I' : '-',
           cpu->state.z ? 'Z' : '-',
           cpu->state.c ? 'C' : '-');
}

static void cpu_format_stack(struct cpu_t *cpu, char *buffer) {
   *buffer = 0x00;
   for (uint16_t sp = cpu->state.sp; sp != 0xff; sp++) {
      char tmp[8];
      sprintf(tmp, " %.2X", _mem_get_byte_direct(cpu, 0x0100 + sp));
      buffer = strcat(buffer, tmp);
   }
}

static mach_timebase_info_data_t timebase_info;

static uint64_t abs_to_nanos(uint64_t abs) {
   return abs * timebase_info.numer  / timebase_info.denom;
}

static uint64_t nanos_to_abs(uint64_t nanos) {
   return nanos * timebase_info.denom / timebase_info.numer;
}

static int cpu_execute_instruction(struct cpu_t *cpu) {
   /* Trace code - Refactor into its own function or module */
   char trace_instruction[256];
   char trace_state[256];
   char trace_stack[256];

   if (cpu->trace) {
      cpu_format_instruction(cpu, trace_instruction);
   }

   uint64_t start_time = mach_absolute_time();

   /* Fetch instruction */
   cpu_instruction_t *i = &instructions[mem_get_byte(cpu, cpu->state.pc)];

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
      cpu_format_stack(cpu, trace_stack); // TODO: This crashes on the hello world test

      switch (i->bytes) {
         case 1:
            fprintf(stderr, "CPU: %.4X %-20s | %.2X           %-20s  STACK: %s\n",
                    pc, trace_instruction, mem_get_byte(cpu, pc), trace_state, trace_stack);
            break;
         case 2:
            fprintf(stderr, "CPU: %.4X %-20s | %.2X %.2X        %-20s  STACK: %s\n",
                    pc, trace_instruction, mem_get_byte(cpu, pc), mem_get_byte(cpu, pc+1), trace_state, trace_stack);
            break;
         case 3:
            fprintf(stderr, "CPU: %.4X %-20s | %.2X %.2X %.2X     %-20s  STACK: %s\n",
                    pc, trace_instruction, mem_get_byte(cpu, pc), mem_get_byte(cpu, pc+1), mem_get_byte(cpu, pc+2), trace_state, trace_stack);
            break;
      }
   }

   /* Delay */

   if (timebase_info.denom == 0) {
      (void) mach_timebase_info(&timebase_info);
   }

   uint64_t now = mach_absolute_time();

   uint64_t elapsed_nano = abs_to_nanos(now - start_time);
   uint64_t expected_duration = (i->cycles * (1000000000 / 960000));
   uint64_t delay_nano = expected_duration - elapsed_nano;

   //fprintf(stderr, "Expected: %" PRId64 " Elapsed: %" PRId64 " Delay: %" PRId64 "\n", expected_duration, elapsed_nano, delay_nano);

   mach_wait_until(now + nanos_to_abs(delay_nano));

   return i->opcode;
}

/* Public API */

void cpu_init(struct cpu_t *cpu) {
   memset(cpu, 0x00, sizeof(struct cpu_t));
}

void cpu_add_mem(struct cpu_t *cpu, struct mem_t *mem) {
  if (cpu->mem == NULL) {
    cpu->mem = mem;
    mem->next = NULL;
  } else {
    mem->next = cpu->mem;
    cpu->mem = mem;
  }

  // If this is RAM mapped to the zero-page and to the stack then we
  // keep a shortcut to it so that we can do direct and fast access
  // with our _mem_get/set_byte/word_direct functions.
  //
  // This makes two assumptions: when RAM is added, it covers both
  // pages. And that mem->obj points to a block of memory. This is
  // fine for the Apple I and Apple II emulators.

  if (mem->type == MEM_TYPE_RAM) {
    if (mem->start == 0x0000 && mem->length >= 0x200) {
      cpu->memory = mem->obj;
    }
  }
}

// RAM Memory

static uint8_t _ram_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return ((uint8_t*) mem->obj)[addr - mem->start];
}

static void _ram_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
  ((uint8_t*) mem->obj)[addr - mem->start] = b;
}

void cpu_add_ram(struct cpu_t *cpu, uint16_t start, uint16_t length) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  mem->type = MEM_TYPE_RAM;
  mem->obj = malloc(length);
  mem->start = start;
  mem->length = length;
  mem->read_handler = _ram_read;
  mem->write_handler = _ram_write;
  mem->next = NULL;
  cpu_add_mem(cpu, mem);
}

// ROM Memory

static uint8_t _rom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return ((uint8_t*) mem->obj)[addr - mem->start];
}

void cpu_add_rom(struct cpu_t *cpu, uint16_t start, uint16_t length, uint8_t *data) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  mem->type = MEM_TYPE_ROM;
  mem->obj = data;
  mem->start = start;
  mem->length = length;
  mem->read_handler = _rom_read;
  mem->write_handler = NULL;
  mem->next = NULL;
  cpu_add_mem(cpu, mem);
}

// IO Memory

void cpu_add_iom(struct cpu_t *cpu, uint16_t start, uint16_t length, void *obj, mem_read_handler_t read_handler, mem_write_handler_t write_handler) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  mem->type = MEM_TYPE_IOM;
  mem->obj = obj;
  mem->start = start;
  mem->length = length;
  mem->read_handler = read_handler;
  mem->write_handler = write_handler;
  mem->next = NULL;
  cpu_add_mem(cpu, mem);
}

void cpu_trace(struct cpu_t *cpu, uint8_t trace) {
   cpu->trace = trace;
}

void cpu_reset(struct cpu_t *cpu) {
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_RES);
   fprintf(stderr, "CPU: cpu->state.pc = %.4x\n", cpu->state.pc);
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

void cpu_irq(struct cpu_t *cpu) {
  _cpu_push_word(cpu, cpu->state.pc);
  _cpu_push_byte(cpu, _cpu_get_status(cpu));
  cpu->state.i = 1;
  cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_IRQ);
}

void cpu_nmi(struct cpu_t *cpu) {
  _cpu_push_word(cpu, cpu->state.pc);
  _cpu_push_byte(cpu, _cpu_get_status(cpu));
  cpu->state.i = 1;
  cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_NMI);
}

void cpu_run(struct cpu_t *cpu) {
   uint64_t instruction_count = 0;
   while (cpu_execute_instruction(cpu) != 0x00) {
      /* TODO: Tick? */
      instruction_count++;
   }
   fprintf(stderr, "Executed %" PRId64 " instructions\n", instruction_count);
}

void cpu_boot(struct cpu_t *cpu) {
   cpu_reset(cpu);
   cpu_run(cpu);
}

void cpu_step(struct cpu_t *cpu) {
   cpu_execute_instruction(cpu);
}
