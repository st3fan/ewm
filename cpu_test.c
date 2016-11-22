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
#include <stdbool.h>

#include "cpu.h"
#include "mem.h"

int main(int argc, char **argv) {
   struct cpu_t cpu;
   cpu_init(&cpu);
   cpu_add_ram_file(&cpu, 0x0000, "roms/6502_functional_test.bin");

   cpu_reset(&cpu);
   cpu.state.pc = 0x0400;

   uint16_t last_pc = cpu.state.pc;

   while (true) {
      int ret = cpu_step(&cpu);
      if (ret != 0) {
         fprintf(stderr, "TEST Unexpected error %d\n", ret);
         exit(1);
      }

      // End of the tests is at 0x3399. This is hard coded and not
      // ideal. Is there a better way to detect this?

      if (cpu.state.pc == 0x3399) {
         fprintf(stderr, "TEST Success\n");
         exit(1);
      }

      // We detect a test failure because we are in a branch deadlock,
      // which we can easily detect by remembering the previous pc and
      // then looking at what we are about to execute.

      if (cpu.state.pc == last_pc) {
         uint8_t i = mem_get_byte(&cpu, cpu.state.pc);
         if (i == 0x10 || i == 0x30 || i == 0x50 || i == 0x70 || i == 0x90 || i == 0xb0 || i == 0xd0 || i == 0xf0) {
            if (mem_get_byte(&cpu, cpu.state.pc + 1) == 0xfe) {
               fprintf(stderr, "TEST Failure at 0x%.4x \n", cpu.state.pc);
               exit(1);
            }
         }
      }

      last_pc = cpu.state.pc;
   }
}
