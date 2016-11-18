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

#include "cpu.h"
#include "ins.h"
#include "mem.h"
#include "fmt.h"

void cpu_format_state(struct cpu_t *cpu, char *buffer) {
   sprintf(buffer, "A=%.2X X=%.2X Y=%.2X S=%.2X SP=%.2X %c%c%c%c%c%c%c%c",
           cpu->state.a, cpu->state.x, cpu->state.y, _cpu_get_status(cpu), cpu->state.sp,

           cpu->state.n ? 'N' : '-',
           cpu->state.v ? 'V' : '-',
           '-',
           cpu->state.b ? 'B' : '-',
           cpu->state.d ? 'D' : '-',
           cpu->state.i ? 'I' : '-',
           cpu->state.z ? 'Z' : '-',
           cpu->state.c ? 'C' : '-');
}

void cpu_format_stack(struct cpu_t *cpu, char buffer[764]) {
   if (cpu->state.sp != 0xff) {
      char *p = buffer;
      *p = 0x00;
      p = strcat(p, "[");
      for (uint16_t sp = cpu->state.sp; sp != 0xff; sp++) {
         if (sp != cpu->state.sp) {
            p = strcat(p, " ");
         }
         char tmp[8];
         sprintf(tmp, "%.2X", _mem_get_byte_direct(cpu, 0x0100 + sp));
         p = strcat(p, tmp);
      }
      strcat(p, "]");
   }
}

void cpu_format_instruction(struct cpu_t *cpu, char *buffer) {
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
