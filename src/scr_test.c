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
#include <unistd.h>

#include "cpu.h"
#include "mem.h"
#include "two.h"
#include "scr.h"

typedef void (*test_setup_t)(struct scr_t *scr);
typedef void (*test_run_t)(struct scr_t *scr);


void txt_full_refresh_setup(struct scr_t *scr) {
   scr->two->screen_mode = EWM_A2P_SCREEN_MODE_TEXT;
   scr->two->screen_page = EWM_A2P_SCREEN_PAGE1;

   for (uint16_t a = 0x0400; a <= 0x0bff; a++) {
      uint8_t v = 0xa0 + (rand() % 64);
      mem_set_byte(scr->two->cpu, a, v);
   }
}

void txt_full_refresh_test(struct scr_t *scr) {
   ewm_scr_update(scr, 0, 0);
}

void lgr_full_refresh_setup(struct scr_t *scr) {
   scr->two->screen_mode = EWM_A2P_SCREEN_MODE_GRAPHICS;
   scr->two->screen_page = EWM_A2P_SCREEN_PAGE1;
   scr->two->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_LGR;
   scr->two->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL;

   for (uint16_t a = 0x0400; a <= 0x0bff; a++) {
      uint8_t v = ((rand() % 16) << 4) | (rand() % 16);
      mem_set_byte(scr->two->cpu, a, v);
   }
}

void lgr_full_refresh_test(struct scr_t *scr) {
   ewm_scr_update(scr, 0, 0);
}

void hgr_full_refresh_setup(struct scr_t *scr) {
   scr->two->screen_mode = EWM_A2P_SCREEN_MODE_GRAPHICS;
   scr->two->screen_page = EWM_A2P_SCREEN_PAGE1;
   scr->two->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_HGR;
   scr->two->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL;

   for (uint16_t a = 0x2000; a <= 0x5fff; a++) {
      mem_set_byte(scr->two->cpu, a, rand());
   }
}

void hgr_full_refresh_test(struct scr_t *scr) {
   ewm_scr_update(scr, 0, 0);
}

void test(struct scr_t *scr, char *name, test_setup_t test_setup, test_run_t test_run) {
   test_setup(scr);

   Uint64 start = SDL_GetPerformanceCounter();
   for (int i = 0; i < 1000; i++) {
      SDL_SetRenderDrawColor(scr->renderer, 0, 0, 0, 255);
      SDL_RenderClear(scr->renderer);

      test_run(scr);

      SDL_Texture *texture = SDL_CreateTextureFromSurface(scr->renderer, scr->surface);
      if (texture != NULL) {
         SDL_RenderCopy(scr->renderer, texture, NULL, NULL);
         SDL_DestroyTexture(texture);
      }

      SDL_RenderPresent(scr->renderer);
   }
   Uint64 now = SDL_GetPerformanceCounter();
   double total = (double)((now - start)*1000) / SDL_GetPerformanceFrequency();
   double per_screen = total / 1000.0;

   printf("%-20s %.3f/refresh\n", name, per_screen);
}

int main() {
   if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS) < 0) {
      fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Window *window = SDL_CreateWindow("EWM v0.1 - scr_test", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
      EWM_SCR_WIDTH*3, EWM_SCR_HEIGHT*3, SDL_WINDOW_SHOWN);
   if (window == NULL) {
      fprintf(stderr, "Failed create window: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Renderer *renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);
   if (renderer == NULL) {
      fprintf(stderr, "Failed to create renderer: %s\n", SDL_GetError());
      return 1;
   }

   SDL_RenderSetLogicalSize(renderer, EWM_SCR_WIDTH, EWM_SCR_HEIGHT);

   sleep(3); // Is this ok? Seems to be needed to get the window on the screen

   // Setup the CPU, Apple ][+ and it's screen.

   struct ewm_two_t *two = ewm_two_create(EWM_TWO_TYPE_APPLE2PLUS, renderer, NULL);
   cpu_reset(two->cpu);

   test(two->scr, "txt_full_refresh", txt_full_refresh_setup, txt_full_refresh_test);
   test(two->scr, "lgr_full_refresh", lgr_full_refresh_setup, lgr_full_refresh_test);
   test(two->scr, "hgr_full_refresh", hgr_full_refresh_setup, hgr_full_refresh_test);

   // Destroy DSL things

   SDL_DestroyWindow(window);
   SDL_DestroyRenderer(renderer);
   SDL_Quit();

   return 0;
}
