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
#include <string.h>
#include <stdint.h>
#include <inttypes.h>
#include <stdlib.h>
#include <unistd.h>

#include <mach/mach.h>
#include <mach/mach_time.h>

#include "cpu.h"
#include "ins.h"

/* Private API */

typedef void (*cpu_instruction_handler_t)(struct cpu_t *cpu);
typedef void (*cpu_instruction_handler_byte_t)(struct cpu_t *cpu, uint8_t oper);
typedef void (*cpu_instruction_handler_word_t)(struct cpu_t *cpu, uint16_t oper);

// The following get and set memory directly. There are no checks, so
// make sure you are doing the right thing. Mainly used for managing
// the stack, reading instructions, reading vectors and tracing code.

uint8_t _mem_get_byte(struct cpu_t *cpu, uint16_t addr) {
   return cpu->memory[addr];
}

uint16_t _mem_get_word(struct cpu_t *cpu, uint16_t addr) {
   return *((uint16_t*) &cpu->memory[addr]);
}

void _mem_set_byte(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
  cpu->memory[addr] = v;
}

void _mem_set_word(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
  *((uint16_t*) &cpu->memory[addr]) = v;
}

// Stack management.

void _cpu_push_byte(struct cpu_t *cpu, uint8_t b) {
  _mem_set_byte(cpu, 0x0100 + cpu->state.sp, b);
  cpu->state.sp -= 1;
}

void _cpu_push_word(struct cpu_t *cpu, uint16_t w) {
  _cpu_push_byte(cpu, (uint8_t) (w >> 8));
  _cpu_push_byte(cpu, (uint8_t) w);
}

uint8_t _cpu_pull_byte(struct cpu_t *cpu) {
  cpu->state.sp += 1;
  return _mem_get_byte(cpu, 0x0100 + cpu->state.sp);
}

uint16_t _cpu_pull_word(struct cpu_t *cpu) {
  return (uint16_t) _cpu_pull_byte(cpu) | ((uint16_t) _cpu_pull_byte(cpu) << 8);
}

#if 1
static void cpu_format_instruction(struct cpu_t *cpu, char *buffer) {
   *buffer = 0x00;

   cpu_instruction_t *i = &instructions[cpu->memory[cpu->state.pc]];
   uint8_t opcode = cpu->memory[cpu->state.pc];

   /* Single byte instructions */
   if (i->bytes == 1) {
      sprintf(buffer, "%s", i->name);
   }

   /* JSR is the only exception */
   else if (opcode == 0x20) {
      sprintf(buffer, "%s $%.4X", i->name, _mem_get_word(cpu, cpu->state.pc+1));
   }

   /* Branches */
   else if ((opcode & 0b00011111) == 0b00010000) {
      int8_t offset = (int8_t) _mem_get_byte(cpu, cpu->state.pc+1);
      uint16_t addr = cpu->state.pc + 2 + offset;
      sprintf(buffer, "%s $%.4X", i->name, addr);
   }

   else if ((opcode & 0b00000011) == 0b00000001) {
      switch ((opcode & 0b00011100) >> 2) {
         case 0b000:
            sprintf(buffer, "%s ($%.2X,X)", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b001:
            sprintf(buffer, "%s $%.2X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b010:
            sprintf(buffer, "%s #$%.2X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b011:
            sprintf(buffer, "%s $%.2X%.2X", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
            break;
         case 0b100:
            sprintf(buffer, "%s ($%.2X),Y", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b101:
            sprintf(buffer, "%s $%.2X,X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b110:
            sprintf(buffer, "%s $%.2X%.2X,Y", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
            break;
         case 0b111:
            sprintf(buffer, "%s $%.2X%.2X,X", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
            break;
      }
   }

   else if ((opcode & 0b00000011) == 0b00000010) {
      switch ((opcode & 0b00011100) >> 2) {
         case 0b000:
            sprintf(buffer, "%s #$%.2X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b001:
            sprintf(buffer, "%s $%.2X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b010:
            sprintf(buffer, "%s", i->name);
            break;
         case 0b011:
            sprintf(buffer, "%s $%.2X%.2X", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
            break;
         case 0b101:
            sprintf(buffer, "%s $%.2X,X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b111:
            sprintf(buffer, "%s $%.2X%.2X,X", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
            break;
      }
   }

   else if ((opcode & 0b00000011) == 0b00000000) {
      switch ((opcode & 0b00011100) >> 2) {
         case 0b000:
            sprintf(buffer, "%s #$%.2X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b001:
            sprintf(buffer, "%s $%.2X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b011:
            sprintf(buffer, "%s $%.2X%.2X", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
            break;
         case 0b101:
            sprintf(buffer, "%s $%.2X,X", i->name, cpu->memory[cpu->state.pc+1]);
            break;
         case 0b111:
            sprintf(buffer, "%s $%.2X%.2X,X", i->name, cpu->memory[cpu->state.pc+2], cpu->memory[cpu->state.pc+1]);
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
      sprintf(tmp, " %.2X", _mem_get_byte(cpu, 0x0100 + sp));
      buffer = strcat(buffer, tmp);
   }
}
#endif

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
   cpu_instruction_t *i = &instructions[cpu->memory[cpu->state.pc]];

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
         ((cpu_instruction_handler_byte_t) i->handler)(cpu, _mem_get_byte(cpu, pc+1));
         break;
      case 3:
         ((cpu_instruction_handler_word_t) i->handler)(cpu, _mem_get_word(cpu, pc+1));
         break;
   }


   if (cpu->trace) {
      cpu_format_state(cpu, trace_state);
      cpu_format_stack(cpu, trace_stack); // TODO: This crashes on the hello world test

      switch (i->bytes) {
         case 1:
            fprintf(stderr, "CPU: %.4X %-20s | %.2X           %-20s  STACK: %s\n",
                    pc, trace_instruction, cpu->memory[pc], trace_state, trace_stack);
            break;
         case 2:
            fprintf(stderr, "CPU: %.4X %-20s | %.2X %.2X        %-20s  STACK: %s\n",
                    pc, trace_instruction, cpu->memory[pc], cpu->memory[pc+1], trace_state, trace_stack);
            break;
         case 3:
            fprintf(stderr, "CPU: %.4X %-20s | %.2X %.2X %.2X     %-20s  STACK: %s\n",
                    pc, trace_instruction, cpu->memory[pc], cpu->memory[pc+1], cpu->memory[pc+2], trace_state, trace_stack);
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

static void iom_init(struct iom_t *iom, uint16_t start, uint16_t length, void *obj, iom_read_handler_t read_handler, iom_write_handler_t write_handler) {
   memset(iom, 0x00, sizeof(struct iom_t));
   iom->start = start;
   iom->length = length;
   iom->obj = obj;
   iom->read_handler = read_handler;
   iom->write_handler = write_handler;
}

/* Public API */

void cpu_init(struct cpu_t *cpu) {
   memset(cpu, 0x00, sizeof(struct cpu_t));
   cpu->memory = malloc(64 * 1024);
}

void cpu_add_ram(struct cpu_t *cpu, uint16_t start, uint16_t length, uint8_t *data) {
   if (data != NULL) {
      memcpy(cpu->memory + start, data, length);
   }
   /* TODO: Mark pages as RAM */
}

void cpu_add_rom(struct cpu_t *cpu, uint16_t start, uint16_t length, uint8_t *data) {
   if (data != NULL) {
      memcpy(cpu->memory + start, data, length);
   }
   /* TODO: Mark pages as ROM */
}

void cpu_add_iom(struct cpu_t *cpu, uint16_t start, uint16_t length, void *obj, iom_read_handler_t read_handler, iom_write_handler_t write_handler) {
   struct iom_t *iom = (struct iom_t*) malloc(sizeof(struct iom_t));
   iom_init(iom, start, length, obj, read_handler, write_handler);
   iom->next = (struct iom_t*) cpu->iom;
   cpu->iom = iom;
   /* TODO: Mark pages as IO */
}

void cpu_trace(struct cpu_t *cpu, uint8_t trace) {
   cpu->trace = trace;
}

void cpu_reset(struct cpu_t *cpu) {
   cpu->state.pc = _mem_get_word(cpu, 0xfffc);
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
   /* TODO: Implement support for IRQ triggering */
}

void cpu_nmi(struct cpu_t *cpu) {
   /* TODO: Implement support for interrupt triggering */
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
