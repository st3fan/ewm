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

#include <stdint.h>
#include <stdio.h>

#include "fmt.h"
#include "cpu.h"
#include "mem.h"

//
// This implements the "Doing something more interactive with it" part of the code golf at
// https://codegolf.stackexchange.com/questions/12844/emulate-a-mos-6502-cpu
//

uint8_t input_handler(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return 0;
}

void output_handler(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
  printf("Hello\n");
  if (addr == 0xf001) {
    printf("Got: %c\n", b);
  }
}

int main() {
  struct cpu_t *cpu = cpu_create(EWM_CPU_MODEL_6502);
  cpu_add_ram(cpu, 0x0000, 0xbfff);
  cpu_add_ram_file(cpu, 0xc000, "rom/ehbasic.bin");
  cpu_add_iom(cpu, 0xf000, 0xf0ff, NULL, input_handler, output_handler);
  cpu_reset(cpu);
  cpu->state.pc = 0xc000;
  
  while (true) {
    char state[1024];
    cpu_format_state(cpu, state);

    char ins[1024];
    cpu_format_instruction(cpu, ins);
    
    printf("%.4X: %s %s\n", cpu->state.pc, state, ins);
    
    int ret = cpu_step(cpu);
    if (ret < 0) {
      fprintf(stderr, "Unexpected error %d\n", ret);
      return 1;
    }
  }
  
  return 0;
}
