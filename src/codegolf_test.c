// The MIT License (MIT)
//
// Copyright (c) 2020 Stefan Arentz - http://github.com/st3fan/ewm
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

#include "cpu.h"
#include "mem.h"

//
// This implements the "Testing your emulator" part of the code golf at
// https://codegolf.stackexchange.com/questions/12844/emulate-a-mos-6502-cpu
//

int main() {
  struct cpu_t *cpu = cpu_create(EWM_CPU_MODEL_6502);
  cpu_add_ram(cpu, 0x0000, 0x3FFF);
  cpu_add_ram_file(cpu, 0x4000, "rom/AllSuiteA.bin");
  cpu_reset(cpu);
  cpu->state.pc = 0x4000;  
  
  while (true) {
    int ret = cpu_step(cpu);
    if (ret < 0) {
      fprintf(stderr, "Unexpected error %d\n", ret);
      return 1;
    }

    if (cpu->state.pc == 0x45c0) {
      fprintf(stderr, "Test finished with status 0x%.2x\n", mem_get_byte(cpu, 0x0210));
      break;
    }    
  }
  
  return 0;
}
