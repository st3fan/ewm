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

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <stdint.h>
#include <time.h>

#include "cpu.h"
#include "mem.h"
#include "utl.h"
#include "lua.h"

int test(int model, uint16_t start_addr, uint16_t success_addr, char *rom_path) {
   struct cpu_t *cpu = cpu_create(model);
   cpu_add_ram_file(cpu, 0x0000, rom_path);
   cpu_reset(cpu);
   cpu->state.pc = start_addr;

#if 1
   cpu->lua = ewm_lua_create();
   ewm_cpu_init_lua(cpu, cpu->lua);

   if (ewm_lua_load_script(cpu->lua, "cpu_test.lua") != 0) {
      printf("Lua script failed to load\n"); // TODO Move errors reporting into C code
      exit(1);
   }
#endif

   uint16_t last_pc = cpu->state.pc;

   struct timespec start;
   if (clock_gettime(CLOCK_REALTIME, &start) != 0) {
      perror("Cannot get time");
      exit(1);
   }

   while (true) {
      int ret = cpu_step(cpu);
      if (ret < 0) {
         switch (ret) {
            case EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION:
               fprintf(stderr, "TEST   Unimplemented instruction 0x%.2x at 0x%.4x\n",
                       mem_get_byte(cpu, cpu->state.pc), cpu->state.pc);
               return -1;
            default:
               fprintf(stderr, "TEST   Unexpected error %d\n", ret);
               return -1;
         }
      }

      // End of the tests is at 0x3399. This is hard coded and not
      // ideal. Is there a better way to detect this?

      if (cpu->state.pc == success_addr) {
	 struct timespec now;
	 if (clock_gettime(CLOCK_REALTIME, &now) != 0) {
	    perror("Cannot get time");
	    exit(1);
	 }

	 // Taking a shortcut here because our test will never run so
	 // long that it will overflow an uint64_t

	 uint64_t duration_ms = (now.tv_sec * 1000 + (now.tv_nsec / 1000000))
	    - (start.tv_sec * 1000 + (start.tv_nsec / 1000000));
	 double duration  = (double) duration_ms / 1000.0;
	 double mhz = (double) cpu->counter * (1.0 / duration) / 1000000.0;
	 
	 fprintf(stderr, "TEST   Success; executed %" PRIu64 " cycles in %.4f at %.4f MHz\n",
		 cpu->counter, duration, mhz);

         return 0;
      }

      // We detect a test failure because we are in a branch deadlock,
      // which we can easily detect by remembering the previous pc and
      // then looking at what we are about to execute.

      if (cpu->state.pc == last_pc) {
         uint8_t i = mem_get_byte(cpu, cpu->state.pc);
         if (i == 0x10 || i == 0x30 || i == 0x50 || i == 0x70 || i == 0x90 || i == 0xb0 || i == 0xd0 || i == 0xf0) {
            if (mem_get_byte(cpu, cpu->state.pc + 1) == 0xfe) {
               fprintf(stderr, "TEST   Failure at 0x%.4x \n", cpu->state.pc);
               return -1;
            }
         }
      }

      last_pc = cpu->state.pc;
   }
}

int main(int argc, char **argv) {
   fprintf(stderr, "TEST Running 6502 tests\n");
   test(EWM_CPU_MODEL_6502,  0x0400, 0x3399, "rom/6502_functional_test.bin");
   fprintf(stderr, "TEST Running 65C02 tests\n");
   test(EWM_CPU_MODEL_65C02, 0x0400, 0x24a8, "rom/65C02_extended_opcodes_test.bin");
}
