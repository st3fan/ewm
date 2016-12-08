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
#include <stdint.h>
#include <stdbool.h>

#include "cpu.h"
#include "a2p.h"
#include "scr.h"
#include "mem.h"

#include "sdl.h"

// TODO Remove these globals - Do we need a struct ewm_t?

SDL_Window *window = NULL;
SDL_Renderer *renderer = NULL;

void sdl_initialize() {
  if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS) < 0) {
    fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
    exit(1);
  }

  //

  window = SDL_CreateWindow("Test", 400, 60, 280*3, 192*3, SDL_WINDOW_SHOWN);
  if (window == NULL) {
    fprintf(stderr, "Failed create window: %s\n", SDL_GetError());
    exit(1);
  }

  renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);
  if (renderer == NULL) {
    fprintf(stderr, "Failed to create renderer: %s\n", SDL_GetError());
    exit(1);
  }
}

Uint32 my_callbackfunc(Uint32 interval, void *param) {
   SDL_Event event;
   SDL_UserEvent userevent;

   userevent.type = SDL_USEREVENT;
   userevent.code = 0;
   userevent.data1 = NULL;
   userevent.data2 = NULL;

   event.type = SDL_USEREVENT;
   event.user = userevent;

   SDL_PushEvent(&event);

   return interval;
}

void sdl_main(struct cpu_t *cpu, struct a2p_t *a2p, struct scr_t *scr) {
   bool quit = false;

   int fps = 0;
   Uint32 next_time = SDL_GetTicks() + (1000 / 50);

   SDL_StartTextInput();

   (void) SDL_AddTimer(1000, my_callbackfunc, NULL);

   while (quit == false)
   {
      // Events

      SDL_Event event;
      while (SDL_PollEvent(&event) != 0) {
         switch (event.type) {
            case SDL_QUIT:
               quit = true;
               break;
            case SDL_KEYDOWN:
               if (event.key.keysym.mod & KMOD_CTRL) {
                  if (event.key.keysym.sym >= SDLK_a && event.key.keysym.sym <= SDLK_z) {
                     a2p->key = (event.key.keysym.sym - SDLK_a) | 0x80;
                  } else {
                     // TODO Implement control codes 1b - 1f
                  }
               } else if (event.key.keysym.mod & KMOD_GUI) {
                  switch (event.key.keysym.sym) {
                     case SDLK_ESCAPE:
                          fprintf(stderr, "[SDL] Reset\n");
                        cpu_reset(cpu);
                        break;
                  }
               } else if (event.key.keysym.mod == KMOD_NONE) {
                  switch (event.key.keysym.sym) {
                     case SDLK_RETURN:
                        a2p->key = 0x0d | 0x80; // CR
                        break;
                     case SDLK_TAB:
                        a2p->key = 0x09 | 0x80; // HT
                     case SDLK_DELETE:
                        a2p->key = 0x7f | 0x80; // DEL
                        break;
                     case SDLK_LEFT:
                        a2p->key = 0x08 | 0x80; // BS
                        break;
                     case SDLK_RIGHT:
                        a2p->key = 0x15 | 0x80; // NAK
                        break;
                     case SDLK_UP:
                        a2p->key = 0x0b | 0x80; // VT
                        break;
                     case SDLK_DOWN:
                        a2p->key = 0x0a | 0x80; // LF
                        break;
                     case SDLK_ESCAPE:
                        a2p->key = 0x1b | 0x80; // ESC
                        break;
                  }
               }
               break;
            case SDL_TEXTINPUT:
               if (strlen(event.text.text) == 1) {
                  a2p->key = toupper(event.text.text[0]) | 0x80;
               }
               break;
            case SDL_USEREVENT:
               printf("fps = %d\n", fps);
               fps = 0;
               break;
         }
      }

      // Logic
      for (int i = 0; i < 5000; i++) {
         int ret = cpu_step(cpu);
         if (ret != 0) {
            switch (ret) {
               case EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION:
                  fprintf(stderr, "CPU: Exited because of unimplemented instructions 0x%.2x at 0x%.4x\n",
                          mem_get_byte(cpu, cpu->state.pc), cpu->state.pc);
                  break;
               case EWM_CPU_ERR_STACK_OVERFLOW:
                  fprintf(stderr, "CPU: Exited because of stack overflow at 0x%.4x\n", cpu->state.pc);
                  break;
               case EWM_CPU_ERR_STACK_UNDERFLOW:
                  fprintf(stderr, "CPU: Exited because of stack underflow at 0x%.4x\n", cpu->state.pc);
                  break;
            }

            cpu_nmi(cpu);

            //exit(1);
         }
      }

      SDL_Delay(10);

      // Render

      if (a2p->screen_dirty) {
         if (SDL_GetTicks() >= next_time) {
            ewm_scr_update(scr);
            a2p->screen_dirty = false;
            SDL_RenderPresent(scr->renderer);
            fps++;

            next_time = SDL_GetTicks() + (1000 / 50);
         }
      }
   }

   printf("sdl_main done\n");

   SDL_DestroyWindow(window);
   SDL_Quit();
}
