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

#include <string.h>

#include <SDL2/SDL.h>

#include "tty.h"
#include "sdl.h"
#include "boo.h"

static char *menu[24] = {
   "****************************************",
   "*                                      *",
   "*       _______ ________ _______       *",
   "*      !    ___!  !  !  !   !   !      *",
   "*      !    ___!  !  !  !       !      *",
   "*      !_______!________!__!_!__!      *",
   "*                                      *",
   "*        GITHUB.COM/ST3FAN/EWM         *",
   "*                                      *",
   "* WHAT WOULD YOU LIKE TO EMULATE?      *",
   "*                                      *",
   "*   1) APPLE 1                         *",
   "*      6502 / 8KB / WOZ MONITOR        *",
   "*                                      *",
   "*   2) REPLICA 1                       *",
   "*      65C02 / 48KB / KRUSADER         *",
   "*                                      *",
   "*   3) APPLE ][+                       *",
   "*      6502 / 64KB (LANGUAGE CARD)     *",
   "*      DISK II / AUTOSTART ROM         *",
   "*                                      *",
   "* START WITH --HELP TO SEE ALL OPTIONS *",
   "*                                      *",
   "****************************************"
};

int ewm_boo_main(int argc, char **argv) {
   // Setup SDL

   if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS) < 0) {
      fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Window *window = SDL_CreateWindow("EWM v0.1 - Bootloader", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
         280*3, 192*3, SDL_WINDOW_SHOWN);
   if (window == NULL) {
      fprintf(stderr, "Failed create window: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Renderer *renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);
   if (renderer == NULL) {
      fprintf(stderr, "Failed to create renderer: %s\n", SDL_GetError());
      return 1;
   }

   if (ewm_sdl_check_renderer(renderer) != 0) {
      fprintf(stderr, "ewm: boo: unsupported renderer\n");
      return 1;
   }

   SDL_RenderSetLogicalSize(renderer, 280, 192);

   // We only need a tty to display the menu

   SDL_Color green = {255,255,0,255};
   struct ewm_tty_t *tty = ewm_tty_create(renderer, green);

   // Main loop

   uint32_t ticks = SDL_GetTicks();
   uint32_t phase = 1;

   int result = -1;

   while (result == -1) {
      // Handle the next event

      SDL_Event event;
      while (SDL_PollEvent(&event) != 0) {
         switch (event.type) {
            case SDL_QUIT:
               result = EWM_BOO_QUIT;
               break;
            case SDL_KEYDOWN:
               switch (event.key.keysym.sym) {
                  case SDLK_1:
                     result = EWM_BOO_BOOT_APPLE1;
                     break;
                  case SDLK_2:
                     result = EWM_BOO_BOOT_REPLICA1;
                     break;
                  case SDLK_3:
                     result = EWM_BOO_BOOT_APPLE2PLUS;
                     break;
               }
               break;
         }
      }

      // If we are done, exit

      if (result != -1) {
         break;
      }

      // Update the screen

      if ((SDL_GetTicks() - ticks) >= (1000 / EWM_BOO_FPS)) {
         if (tty->screen_dirty || (phase == 0) || ((phase % (EWM_BOO_FPS / 4)) == 0)) {
            SDL_SetRenderDrawColor(tty->renderer, 0, 0, 0, 255);
            SDL_RenderClear(tty->renderer);

            for (int i = 0; i < 24; i++) {
               char *p = (char*) tty->screen_buffer;
               p += (i * 40);
               memcpy(p, menu[i], 40);
            }

            tty->screen_cursor_column = 34;
            tty->screen_cursor_row = 9;

            ewm_tty_refresh(tty, phase, EWM_BOO_FPS);
            tty->screen_dirty = false;

            SDL_Texture *texture = SDL_CreateTextureFromSurface(tty->renderer, tty->surface);
            if (texture != NULL) {
               SDL_RenderCopy(tty->renderer, texture, NULL, NULL);
               SDL_DestroyTexture(texture);
            }

            SDL_RenderPresent(tty->renderer);
         }

         ticks = SDL_GetTicks();

         phase += 1;
         if (phase == EWM_BOO_FPS) {
            phase = 0;
         }
      }
   }

   // Destroy SDL

   SDL_DestroyWindow(window);
   SDL_DestroyRenderer(renderer);
   SDL_Quit();

   return result;
}

