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

#ifndef EWM_TWO_H
#define EWM_TWO_H

#include <stdint.h>

#include <SDL2/SDL.h>

#define EWM_TWO_TYPE_APPLE2     0
#define EWM_TWO_TYPE_APPLE2PLUS 1
#define EWM_TWO_TYPE_APPLE2E    2

#define EWM_A2P_SCREEN_MODE_TEXT 0
#define EWM_A2P_SCREEN_MODE_GRAPHICS 1

#define EWM_A2P_SCREEN_GRAPHICS_MODE_LGR 0
#define EWM_A2P_SCREEN_GRAPHICS_MODE_HGR 1

#define EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL 0
#define EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED 1

#define EWM_A2P_SCREEN_PAGE1 0
#define EWM_A2P_SCREEN_PAGE2 1

#define EWM_A2P_BUTTON1 0
#define EWM_A2P_BUTTON2 1
#define EWM_A2P_BUTTON3 2
#define EWM_A2P_BUTTON4 3 // Actually ony exists on the gs?
#define EWM_A2P_BUTTON_COUNT 4

struct mem_t;
struct ewm_dsk_t;
struct scr;

struct ewm_two_t {
   int type;
   struct cpu_t *cpu;
   struct scr_t *scr;
   struct ewm_dsk_t *dsk;
   struct ewm_alc_t *alc;

   struct mem_t *ram;
   struct mem_t *roms[6];
   struct mem_t *iom;

   uint8_t key;
   uint8_t buttons[EWM_A2P_BUTTON_COUNT];

   uint64_t padl0_time;
   uint8_t padl0_value;
   uint64_t padl1_time;
   uint8_t padl1_value;
   uint64_t padl2_time; // Are 2 and 3 actually used? Not sure what to map them to.
   uint8_t padl2_value;
   uint64_t padl3_time;
   uint8_t padl3_value;

   SDL_Joystick *joystick;
};

struct ewm_two_t *ewm_two_create(int type, SDL_Renderer *renderer, SDL_Joystick *joystick);
void ewm_two_destroy(struct ewm_two_t *two);

int ewm_two_load_disk(struct ewm_two_t *two, int drive, char *path);

int ewm_two_main(int argc, char **argv);

#endif // EWM_TWO_H

