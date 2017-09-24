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

#include <sys/utsname.h>

#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>

#include "cpu.h"
#include "ins.h"
#include "mem.h"
#include "utl.h"

uint64_t get_iterations() {
   struct utsname name;
   if (uname(&name) == 0) {
      if (strcmp(name.machine, "x86_64") == 0) {
         return 1000 * 1000 * 1000;
      }
      if (strcmp(name.machine, "armv6l") == 0) {
         return 100 * 1000 * 1000;
      }
   }
   return 10 * 1000 * 1000;
}

void test(struct cpu_t *cpu, uint8_t opcode) {
   uint64_t runs[3];

   struct cpu_instruction_t *ins = &cpu->instructions[opcode];

   uint64_t iterations = get_iterations();

   for (int run = 0; run < 3; run++) {
      struct timespec start;
      if (clock_gettime(CLOCK_REALTIME, &start) != 0) {
         perror("Cannot get time");
         exit(1);
      }

      cpu->state.x = 0x12;
      cpu->state.y = 0x12;

      switch (ins->bytes) {
         case 1:
            for (uint64_t i = 0; i < iterations; i++) {
               ((cpu_instruction_handler_t) ins->handler)(cpu);
            }
            break;
         case 2:
            for (uint64_t i = 0; i < iterations; i++) {
               ((cpu_instruction_handler_byte_t) ins->handler)(cpu, 0x12);
            }
            break;
         case 3:
            for (uint64_t i = 0; i < iterations; i++) {
               ((cpu_instruction_handler_word_t) ins->handler)(cpu, 0x1234);
            }
            break;
      }

      struct timespec now;
      if (clock_gettime(CLOCK_REALTIME, &now) != 0) {
         perror("Cannot get time");
         exit(1);
      }

      uint64_t duration_ms = (now.tv_sec * 1000 + (now.tv_nsec / 1000000))
         - (start.tv_sec * 1000 + (start.tv_nsec / 1000000));

      runs[run] = duration_ms;
   }

   printf("$%.2X %s %8llu %8llu %8llu -> %8llu\n",
          opcode, ins->name, runs[0], runs[1], runs[2],
          (runs[0] + runs[1] + runs[2]) / 3);
}

int main(int argc, char **argv) {
   struct cpu_t *cpu = cpu_create(EWM_CPU_MODEL_65C02);
   cpu_add_ram_data(cpu, 0, 0xffff, malloc(0xffff));
   cpu_reset(cpu);

   if (argc > 1) {
      for (int i = 1; i < argc; i++) {
         for (int opcode = 0; opcode <= 255; opcode++) {
            if (strcmp(cpu->instructions[opcode].name, argv[i]) == 0) {
               test(cpu, opcode);
            }
         }
      }
   } else {
      for (int opcode = 0; opcode <= 255; opcode++) {
         test(cpu, opcode);
      }
   }
}
