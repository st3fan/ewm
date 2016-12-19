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

#ifndef EWM_ONE_H
#define EWM_ONE_H

#define EWM_ONE_TYPE_APPLE1   0
#define EWM_ONE_TYPE_REPLICA1 1

#include <SDL2/SDL.h>

struct cpu_t;
struct ewm_tty_t;
struct ewm_pia_t;

struct ewm_one_t {
   int type;
   struct cpu_t *cpu;
   struct ewm_tty_t *tty;
   struct ewm_pia_t *pia;
};

struct ewm_one_t *ewm_one_create(int type, SDL_Renderer *renderer);
int ewm_one_init(struct ewm_one_t *one, int type, SDL_Renderer *renderer);
void ewm_one_destroy(struct ewm_one_t *one);

int ewm_one_main(int argc, char **argv);

#endif // EWM_ONE_H
