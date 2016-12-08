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

#ifndef EWM_A2P_H
#define EWM_A2P_H

#include <stdint.h>

struct mem_t;
struct ewm_dsk_t;

#define EWM_A2P_SCREEN_MODE_TEXT 0
#define EWM_A2P_SCREEN_MODE_GRAPHICS 1

#define EWM_A2P_SCREEN_GRAPHICS_MODE_LGR 0
#define EWM_A2P_SCREEN_GRAPHICS_MODE_HGR 1

#define EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL 0
#define EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED 1

#define EWM_A2P_SCREEN_PAGE1 0
#define EWM_A2P_SCREEN_PAGE2 1

struct a2p_t {
   struct mem_t *ram;
   struct mem_t *rom;
   struct mem_t *iom;
   struct ewm_dsk_t *dsk;

   uint8_t *screen_txt_data;
   struct mem_t *screen_txt_iom;

   uint8_t *screen_hgr_data;
   struct mem_t *screen_hgr_iom;
   int screen_hgr_page;

   int screen_mode;
   int screen_graphics_mode;
   int screen_graphics_style;
   int screen_page;
   int screen_dirty;

   uint8_t key;
};

void a2p_init(struct a2p_t *a2p, struct cpu_t *cpu);
int a2p_load_disk(struct a2p_t *a2p, int drive, char *path);

#endif
