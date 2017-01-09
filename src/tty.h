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

#ifndef EWM_TTY_H
#define EWM_TTY_H

#include <stdbool.h>
#include <stdint.h>

#include <SDL2/SDL.h>

#define EWM_ONE_TTY_ROWS 24
#define EWM_ONE_TTY_COLUMNS 40
#define EWM_ONE_TTY_CURSOR '@'

struct ewm_chr_t;

struct ewm_tty_t {
   SDL_Renderer *renderer;
   struct ewm_chr_t *chr;
   bool screen_dirty;
   uint8_t screen_buffer[EWM_ONE_TTY_ROWS * EWM_ONE_TTY_COLUMNS];
   int screen_cursor_row;
   int screen_cursor_column;
   int screen_cursor_blink;
};

struct ewm_tty_t *ewm_tty_create(SDL_Renderer *renderer);
void ewm_tty_destroy(struct ewm_tty_t *tty);
void ewm_tty_write(struct ewm_tty_t *tty, uint8_t v);
void ewm_tty_reset(struct ewm_tty_t *tty);
void ewm_tty_refresh(struct ewm_tty_t *tty, uint32_t phase, uint32_t fps);

#endif // EWM_TTY_H
