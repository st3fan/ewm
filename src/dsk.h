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

#ifndef EWM_DSK_H
#define EWM_DSK_H

#include <stdbool.h>
#include <stdint.h>

#if defined(EWM_LUA)
#include "lua.h"
#endif

struct cpu_t;
struct mem_t;

#define EWM_DSK_DRIVE1 (0)
#define EWM_DSK_DRIVE2 (1)

#define EWM_DSK_TRACKS (35)
#define EWM_DSK_SECTORS (16)
#define EWM_DSK_SECTOR_SIZE (256)
#define EWM_DSK_NIBBLES_PER_TRACK (6656)

struct ewm_dsk_track_t {
   int length;
   uint8_t *data;
};

struct ewm_dsk_drive_t {
   bool loaded;
   uint8_t volume;
   int track, head, phase;
   bool readonly;
   bool dirty;
   struct ewm_dsk_track_t tracks[EWM_DSK_TRACKS];
};

struct ewm_dsk_t {
   struct mem_t *rom;
   struct mem_t *iom;
   bool on;
   int active_drive;
   int mode;
   uint8_t latch;
   struct ewm_dsk_drive_t drives[2];
   uint8_t drive; // 0 based
   int skip;
#if defined(EWM_LUA)
   struct ewm_lua_t *lua;
#endif
};

#define EWM_DSK_TYPE_UNKNOWN (-1)
#define EWM_DSK_TYPE_DO (0)
#define EWM_DSK_TYPE_PO (1)
#define EWM_DSK_TYPE_NIB (2)

struct ewm_dsk_t *ewm_dsk_create(struct cpu_t *cpu);
int ewm_dsk_set_disk_data(struct ewm_dsk_t *dsk, uint8_t index, bool readonly, void *data, size_t length, int type);
int ewm_dsk_set_disk_file(struct ewm_dsk_t *dsk, uint8_t index, bool readonly, char *path);

#if defined(EWM_LUA)
int ewm_dsk_init_lua(struct ewm_dsk_t *dsk, struct ewm_lua_t *lua);
#endif

#endif
