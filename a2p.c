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

#include "dsk.h"
#include "alc.h"
#include "a2p.h"

#define EWM_A2P_SS_KBD                  0xc000
#define EWM_A2P_SS_KBDSTRB              0xc010
#define EWM_A2P_SS_SPKR                 0xc030

#define EWM_A2P_SS_SCREEN_MODE_GRAPHICS 0xc050
#define EWM_A2P_SS_SCREEN_MODE_TEXT     0xc051
#define EWM_A2P_SS_GRAPHICS_STYLE_FULL  0xc052
#define EWM_A2P_SS_GRAPHICS_STYLE_MIXED 0xc053
#define EWM_A2P_SS_SCREEN_PAGE1         0xc054
#define EWM_A2P_SS_SCREEN_PAGE2         0xc055
#define EWM_A2P_SS_GRAPHICS_MODE_LGR    0xc056
#define EWM_A2P_SS_GRAPHICS_MODE_HGR    0xc057

#define EWM_A2P_SS_SETAN0  0xc058
#define EWM_A2P_SS_CLRAN0  0xc059
#define EWM_A2P_SS_SETAN1  0xc05a
#define EWM_A2P_SS_CLRAN1  0xc05b
#define EWM_A2P_SS_SETAN2  0xc05c
#define EWM_A2P_SS_CLRAN2  0xc05d
#define EWM_A2P_SS_SETAN3  0xc05e
#define EWM_A2P_SS_CLRAN3  0xc05f

#define EWM_A2P_SS_PB0 0xC061
#define EWM_A2P_SS_PB1 0xC062
#define EWM_A2P_SS_PB2 0xC063
#define EWM_A2P_SS_PB3 0xC060 // TODO On the gs only?

uint8_t a2p_iom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   switch (addr) {
      case EWM_A2P_SS_KBD:
         return a2p->key;
      case EWM_A2P_SS_KBDSTRB:
         a2p->key &= 0x7f;
         return 0x00;

      case EWM_A2P_SS_SCREEN_MODE_GRAPHICS:
         a2p->screen_mode = EWM_A2P_SCREEN_MODE_GRAPHICS;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_MODE_TEXT:
         a2p->screen_mode = EWM_A2P_SCREEN_MODE_TEXT;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_MODE_LGR:
         a2p->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_LGR;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_MODE_HGR:
         a2p->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_HGR;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_STYLE_FULL:
         a2p->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_STYLE_MIXED:
         a2p->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_SCREEN_PAGE1:
         a2p->screen_page = EWM_A2P_SCREEN_PAGE1;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_PAGE2:
         a2p->screen_page = EWM_A2P_SCREEN_PAGE2;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_SPKR:
         // TODO Implement speaker support
         break;

      case EWM_A2P_SS_PB0:
         return a2p->buttons[0];
      case EWM_A2P_SS_PB1:
         return a2p->buttons[1];
      case EWM_A2P_SS_PB2:
         return a2p->buttons[2];
      case EWM_A2P_SS_PB3:
         return a2p->buttons[3];

      case EWM_A2P_SS_SETAN0:
         break;
      case EWM_A2P_SS_SETAN1:
         break;
      case EWM_A2P_SS_SETAN2:
         break;
      case EWM_A2P_SS_SETAN3:
         break;

      case EWM_A2P_SS_CLRAN0:
         break;
      case EWM_A2P_SS_CLRAN1:
         break;
      case EWM_A2P_SS_CLRAN2:
         break;
      case EWM_A2P_SS_CLRAN3:
         break;

      default:
         printf("[A2P] Unexpected read at $%.4X\n", addr);
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

      case EWM_A2P_SS_SCREEN_MODE_GRAPHICS:
         a2p->screen_mode = EWM_A2P_SCREEN_MODE_GRAPHICS;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_MODE_TEXT:
         a2p->screen_mode = EWM_A2P_SCREEN_MODE_TEXT;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_MODE_LGR:
         a2p->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_LGR;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_MODE_HGR:
         a2p->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_HGR;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_STYLE_FULL:
         a2p->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_STYLE_MIXED:
         a2p->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_SCREEN_PAGE1:
         a2p->screen_page = EWM_A2P_SCREEN_PAGE1;
         a2p->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_PAGE2:
         a2p->screen_page = EWM_A2P_SCREEN_PAGE2;
         a2p->screen_dirty = true;
         break;

      case EWM_A2P_SS_SETAN0:
         break;
      case EWM_A2P_SS_SETAN1:
         break;
      case EWM_A2P_SS_SETAN2:
         break;
      case EWM_A2P_SS_SETAN3:
         break;

      case EWM_A2P_SS_CLRAN0:
         break;
      case EWM_A2P_SS_CLRAN1:
         break;
      case EWM_A2P_SS_CLRAN2:
         break;
      case EWM_A2P_SS_CLRAN3:
         break;

      default:
         printf("[A2P] Unexpected write at $%.4X\n", addr);
         break;
   }
}

uint8_t a2p_screen_txt_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   return a2p->screen_txt_data[addr - mem->start];
}

void a2p_screen_txt_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   a2p->screen_txt_data[addr - mem->start] = b;
   a2p->screen_dirty = true;
}

uint8_t a2p_screen_hgr_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   return a2p->screen_hgr_data[addr - mem->start];
}

void a2p_screen_hgr_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct a2p_t *a2p = (struct a2p_t*) mem->obj;
   a2p->screen_hgr_data[addr - mem->start] = b;
   a2p->screen_dirty = true;
}

int a2p_init(struct a2p_t *a2p, struct cpu_t *cpu) {
   memset(a2p, 0x00, sizeof(struct a2p_t));

   a2p->cpu = cpu;

   a2p->ram = cpu_add_ram(cpu, 0x0000, 48 * 1024);
   a2p->ram->description = "ram/a2p/$0000";
   a2p->roms[0] = cpu_add_rom_file(cpu, 0xd000, "roms/341-0011.bin"); // AppleSoft BASIC D000
   a2p->roms[1] = cpu_add_rom_file(cpu, 0xd800, "roms/341-0012.bin"); // AppleSoft BASIC D800
   a2p->roms[2] = cpu_add_rom_file(cpu, 0xe000, "roms/341-0013.bin"); // AppleSoft BASIC E000
   a2p->roms[3] = cpu_add_rom_file(cpu, 0xe800, "roms/341-0014.bin"); // AppleSoft BASIC E800
   a2p->roms[4] = cpu_add_rom_file(cpu, 0xf000, "roms/341-0015.bin"); // AppleSoft BASIC E800
   a2p->roms[5] = cpu_add_rom_file(cpu, 0xf800, "roms/341-0020.bin"); // AppleSoft BASIC Autostart Monitor F8000
   a2p->iom = cpu_add_iom(cpu, 0xc000, 0xc07f, a2p, a2p_iom_read, a2p_iom_write);
   a2p->iom->description = "iom/a2p/$C000";

   a2p->dsk = ewm_dsk_create(cpu);

   // TODO Move into a2p_t
   struct ewm_alc_t *alc = ewm_alc_create(cpu);
   if (alc == NULL) {
      fprintf(stderr, "[A2P] Could not create Apple Language Card\n");
      return -1;
   }

   // TODO Introduce ewm_scr_t that captures everything related to the apple 2 screen so that it can be re-used.

   a2p->screen_txt_data = malloc(2 * 1024);
   a2p->screen_txt_iom = cpu_add_iom(cpu, 0x0400, 0x0bff, a2p, a2p_screen_txt_read, a2p_screen_txt_write);
   a2p->screen_txt_iom->description = "ram/a2p/$0400";

   a2p->screen_hgr_data = malloc(16 * 1024);
   a2p->screen_hgr_iom = cpu_add_iom(cpu, 0x2000, 0x5fff, a2p, a2p_screen_hgr_read, a2p_screen_hgr_write);
   a2p->screen_hgr_iom->description = "ram/a2p/$2000";

   return 0;
}

struct a2p_t *a2p_create(struct cpu_t *cpu) {
   struct a2p_t *a2p = malloc(sizeof(struct a2p_t));
   if (a2p_init(a2p, cpu) != 0) {
      free(a2p);
      a2p = NULL;
   }
   return a2p;
}

void a2p_destroy(struct a2p_t *a2p) {
   // TODO
}

int a2p_load_disk(struct a2p_t *a2p, int drive, char *path) {
   return ewm_dsk_set_disk_file(a2p->dsk, drive, false, path);
}
