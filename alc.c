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

#include <stdbool.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "cpu.h"
#include "alc.h"

uint8_t alc_iom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct ewm_alc_t *alc = (struct ewm_alc_t*) mem->obj;

   // Always enable the right banks
   if (addr & 0b00001000) {
      alc->ram1->enabled = true;
      alc->ram2->enabled = false;
      alc->ram3->enabled = true;
   } else {
      alc->ram1->enabled = false;
      alc->ram2->enabled = true;
      alc->ram3->enabled = true;
   }

   switch (addr) {
      // WRTCOUNT = 0, WRITE DISABLE, READ ENABLE
      case 0xc080:
      case 0xc088:
      case 0xc084:
      case 0xc08c:
         alc->wrtcount = 0;
         alc->ram1->flags = MEM_FLAGS_READ;
         alc->ram2->flags = MEM_FLAGS_READ;
         alc->ram3->flags = MEM_FLAGS_READ;
         break;

      // WRTCOUNT++, READ DISABLE, WRITE ENABLE IF WRTCOUNT >= 2
      case 0xc081:
      case 0xc089:
      case 0xc085:
      case 0xc08d:
         alc->wrtcount = alc->wrtcount + 1;
         alc->ram1->flags &= ~MEM_FLAGS_READ;
         alc->ram2->flags &= ~MEM_FLAGS_READ;
         alc->ram3->flags &= ~MEM_FLAGS_READ;
         if (alc->wrtcount >= 2) {
            alc->ram1->flags |= MEM_FLAGS_WRITE;
            alc->ram2->flags |= MEM_FLAGS_WRITE;
            alc->ram3->flags |= MEM_FLAGS_WRITE;
         }
         break;

      // WRTCOUNT = 0, WRITE DISABLE, READ DISABLE
      case 0xc082:
      case 0xc08a:
      case 0xc086:
      case 0xc08e:
         alc->wrtcount = 0;
         alc->ram1->flags &= ~MEM_FLAGS_WRITE;
         alc->ram2->flags &= ~MEM_FLAGS_WRITE;
         alc->ram3->flags &= ~MEM_FLAGS_WRITE;
         alc->ram1->flags &= MEM_FLAGS_WRITE;
         alc->ram2->flags &= MEM_FLAGS_WRITE;
         alc->ram3->flags &= MEM_FLAGS_WRITE;
         break;

      // WRTCOUNT++, READ ENABLE, WRITE ENABLE IF WRTCOUNT >= 2
      case 0xc083:
      case 0xc08b:
      case 0xc087:
      case 0xc08f:
         alc->wrtcount = alc->wrtcount + 1;
         alc->ram1->flags |= MEM_FLAGS_READ;
         alc->ram2->flags |= MEM_FLAGS_READ;
         alc->ram3->flags |= MEM_FLAGS_READ;
         if (alc->wrtcount >= 2) {
            alc->ram1->flags |= MEM_FLAGS_WRITE;
            alc->ram2->flags |= MEM_FLAGS_WRITE;
            alc->ram3->flags |= MEM_FLAGS_WRITE;
         }
         break;

      default:
         fprintf(stderr, "[ALC] Unexpected read at $%.4X\n", addr);
         break;
   }

   return 0;
}

static void alc_iom_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct ewm_alc_t *alc = (struct ewm_alc_t*) mem->obj;

   // Always enable the right banks
   if (addr & 0b00001000) {
      alc->ram1->enabled = true;
      alc->ram2->enabled = false;
      alc->ram3->enabled = true;
   } else {
      alc->ram1->enabled = false;
      alc->ram2->enabled = true;
      alc->ram3->enabled = true;
   }

   switch (addr) {
      // WRTCOUNT = 0, WRITE DISABLE, READ ENABLE
      case 0xc080:
      case 0xc088:
      case 0xc084:
      case 0xc08c:
         alc->wrtcount = 0;
         alc->ram1->flags = MEM_FLAGS_READ;
         alc->ram2->flags = MEM_FLAGS_READ;
         alc->ram3->flags = MEM_FLAGS_READ;
         break;

      // WRTCOUNT = 0, READ DISABLE
      case 0xc081:
      case 0xc089:
      case 0xc085:
      case 0xc08d:
         alc->wrtcount = 0;
         alc->ram1->flags &= ~MEM_FLAGS_READ;
         alc->ram2->flags &= ~MEM_FLAGS_READ;
         alc->ram3->flags &= ~MEM_FLAGS_READ;
         break;

      // WRTCOUNT = 0, WRITE DISABLE, READ DISABLE
      case 0xc082:
      case 0xc08a:
      case 0xc086:
      case 0xc08e:
         alc->wrtcount = 0;
         alc->ram1->flags &= ~MEM_FLAGS_WRITE;
         alc->ram2->flags &= ~MEM_FLAGS_WRITE;
         alc->ram3->flags &= ~MEM_FLAGS_WRITE;
         alc->ram1->flags &= MEM_FLAGS_WRITE;
         alc->ram2->flags &= MEM_FLAGS_WRITE;
         alc->ram3->flags &= MEM_FLAGS_WRITE;
         break;

      // WRTCOUNT = 0, READ ENABLE
      case 0xc083:
      case 0xc08b:
      case 0xc087:
      case 0xc08f:
         alc->wrtcount = 0;
         alc->ram1->flags |= MEM_FLAGS_READ;
         alc->ram2->flags |= MEM_FLAGS_READ;
         alc->ram3->flags |= MEM_FLAGS_READ;
         break;

      default:
         fprintf(stderr, "[ALC] Unexpected write at $%.4X\n", addr);
         break;
   }
}

int ewm_alc_init(struct ewm_alc_t *alc, struct cpu_t *cpu) {
   memset(alc, 0x00, sizeof(struct ewm_alc_t));

   // Order is important. First added is last tried when looking up
   // addresses. So we register the ROM first, which means we never
   // have to disable it.

   alc->rom = cpu_add_rom_file(cpu, 0xf800, "roms/341-0020.bin");
   alc->iom = cpu_add_iom(cpu, 0xc080, 0xc08f, alc, alc_iom_read, alc_iom_write);
   alc->iom->description = "iom/alc/$C080";
   alc->ram1 = cpu_add_ram(cpu, 0xd000, 0xd000 + 4096 - 1);
   alc->ram1->description = "ram/alc/$D000 (RAM1)";
   alc->ram2 = cpu_add_ram(cpu, 0xd000, 0xd000 + 4096 - 1);
   alc->ram2->description = "ram/alc/$D000 (RAM2)";
   alc->ram3 = cpu_add_ram(cpu, 0xe000, 0xe000 + 8192 - 1);
   alc->ram3->description = "ram/alc/$E000 (RAM3)";

   alc->ram1->enabled = false;
   alc->ram2->enabled = false;
   alc->ram3->enabled = false;

   return 0;
}

struct ewm_alc_t *ewm_alc_create(struct cpu_t *cpu) {
   struct ewm_alc_t *alc = malloc(sizeof(struct ewm_alc_t));
   if (ewm_alc_init(alc, cpu) != 0) {
      free(alc);
      alc = NULL;
   }
   return alc;
}
