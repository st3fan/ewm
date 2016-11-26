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

#include <stdlib.h>
#include <string.h>

#include "cpu.h"
#include "mem.h"

#include "a2p.h"

#define EWM_A2P_SS_KBD       0xc000
#define EWM_A2P_SS_KBDSTRB   0xc010
#define EWM_A2P_SS_SPKR      0xc030
#define EWM_A2P_SS_TXTCLR  0xc050
#define EWM_A2P_SS_TXTSET    0xc051
#define EWM_A2P_SS_LOSCR     0xc054
#define EWM_A2P_SS_HISCR     0xc055
#define EWM_A2P_SS_LORES     0xc056
#define EWM_A2P_SS_HIRES   0xc057
#define EWM_A2P_SS_SETAN0    0xc058
#define EWM_A2P_SS_CLRAN0  0xc059
#define EWM_A2P_SS_SETAN1    0xc05a
#define EWM_A2P_SS_CLRAN1  0xc05b
#define EWM_A2P_SS_SETAN2  0xc05c
#define EWM_A2P_SS_CLRAN2    0xc05d
#define EWM_A2P_SS_SETAN3  0xc05e
#define EWM_A2P_SS_CLRAN3    0xc05f

uint8_t a2p_iom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   switch (addr) {
      case EWM_A2P_SS_KBD:
         return a2p->key;
      case EWM_A2P_SS_KBDSTRB:
         a2p->key &= 0x7f;
         return 0x00;

      case EWM_A2P_SS_LOSCR:
         a2p->current_screen = 0;
         a2p->screen1_dirty = true;
         a2p->screen2_dirty = false;
         break;

      case EWM_A2P_SS_HISCR:
         a2p->current_screen = 1;
         a2p->screen1_dirty = false;
         a2p->screen2_dirty = true;
         break;
   }
   return 0;
}

void a2p_iom_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   switch (addr) {
      case EWM_A2P_SS_KBDSTRB:
         a2p->key &= 0x7f;
         break;
   }
}

uint8_t a2p_screen1_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   return a2p->screen1_data[addr - mem->start];
}

void a2p_screen1_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   a2p->screen1_data[addr - mem->start] = b;
   a2p->screen1_dirty = true;
}

uint8_t a2p_screen2_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   return a2p->screen2_data[addr - mem->start];
}

void a2p_screen2_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   a2p->screen2_data[addr - mem->start] = b;
   a2p->screen2_dirty = true;
}

void a2p_init(struct a2p_t *a2p, struct cpu_t *cpu) {
   memset(a2p, sizeof(struct a2p_t), 0x00);

   a2p->ram = cpu_add_ram(cpu, 0x0000, 48 * 1024);
   a2p->rom = cpu_add_rom_file(cpu, 0xd000, "roms/a2p.rom");
   a2p->iom = cpu_add_iom(cpu, 0xc000, 0xc0ff, a2p, a2p_iom_read, a2p_iom_write);

   a2p->screen1_data = malloc(1 * 1024);
   a2p->screen1 = cpu_add_iom(cpu, 0x0400, 0x07ff, a2p, a2p_screen1_read, a2p_screen1_write);

   a2p->screen2_data = malloc(1 * 1024);
   a2p->screen2 = cpu_add_iom(cpu, 0x0800, 0x0bff, a2p, a2p_screen2_read, a2p_screen2_write);
}
