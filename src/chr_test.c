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

#include <stdio.h>
#include <unistd.h>

#include "chr.h"

int main() {
   if (SDL_Init(SDL_INIT_VIDEO) < 0) {
      fprintf(stderr, "[CHR] Failed to initialize SDL: %s\n", SDL_GetError());
      exit(1);
   }

   SDL_Window *window = SDL_CreateWindow("chr_test", SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED, 280*3, 192*3, SDL_WINDOW_SHOWN);
   if (window == NULL) {
      fprintf(stderr, "[CHR] Failed create window: %s\n", SDL_GetError());
      exit(1);
   }

   SDL_SetWindowPosition(window, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED);

   SDL_Renderer *renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_SOFTWARE);
   if (renderer == NULL) {
      fprintf(stderr, "[CHR] Failed to create renderer: %s\n", SDL_GetError());
      exit(1);
   }

   struct ewm_chr_t *chr = ewm_chr_create("rom/3410036.bin", EWM_CHR_ROM_TYPE_2716, renderer);
   if (chr == NULL) {
      fprintf(stderr, "[CHR] Failed to load Character ROM %s\n", "rom/3410036.bin");
      exit(1);
   }

   // TODO Blit the whole character rom as textures and then as surfaces

   return 0;
}
