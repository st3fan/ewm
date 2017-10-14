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

#ifndef EWM_SCR_H
#define EWM_SCR_H

#include <SDL2/SDL.h>

#define EWM_SCR_COLOR_SCHEME_MONOCHROME (0)
#define EWM_SCR_COLOR_SCHEME_COLOR      (1)
#define EWM_SCR_COLOR_SCHEME_DEFAULT    (EWM_SCR_COLOR_SCHEME_MONOCHROME)

#define EWM_SCR_WIDTH (280)
#define EWM_SCR_HEIGHT (192)

struct ewm_two_t;
struct ewm_chr_t;

// The 'scr' object represents the screen. It renders the contents of
// the machine. It has pluggable renders.

struct scr_t {
   struct ewm_two_t *two;
   SDL_Renderer *renderer;
   struct ewm_chr_t *chr;
   int color_scheme;

   uint32_t *pixels;
   SDL_Surface *surface;

   uint32_t *lgr_bitmaps[256];
   uint32_t green;
   uint32_t hgr_colors[6];
};

struct scr_t *ewm_scr_create(struct ewm_two_t *two, SDL_Renderer *renderer);
void ewm_scr_destroy(struct scr_t *scr);
void ewm_scr_update(struct scr_t *scr, int phase, int fps);
void ewm_scr_set_color_scheme(struct scr_t *scr, int color_scheme);

#endif
