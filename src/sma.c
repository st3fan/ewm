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

#include <SDL2/SDL.h>

#include "cpu.h"
#include "mem.h"
#include "tty.h"
#include "sma.h"

#define EWM_SMA_RAM_BASE (0)
#define EWM_SMA_RAM_SIZE (16 * 1024)
#define EWM_SMA_ROM_BASE (EWM_SMA_RAM_BASE + EWM_SMA_RAM_SIZE - 1)
#define EWM_SMA_ROM_SIZE (4 * 1024)
#define EWM_SMA_TTY_BASE (EWM_SMA_ROM_BASE + EWM_SMA_ROM_SIZE - 1)
#define EWM_SMA_TTY_SIZE (1 * 1024)

static bool ewm_sma_poll_event(struct ewm_sma_t *sma, SDL_Window *window) {
   SDL_Event event;
   while (SDL_PollEvent(&event) != 0) {
      switch (event.type) {
         case SDL_QUIT:
            return false;

         case SDL_WINDOWEVENT:
            sma->tty->screen_dirty = true;
            break;

         case SDL_KEYDOWN:
            if (event.key.keysym.mod & KMOD_GUI) {
               switch (event.key.keysym.sym) {
                  case SDLK_ESCAPE:
                     fprintf(stderr, "[SDL] Reset\n");
                     cpu_reset(sma->cpu);
                     break;
                  case SDLK_RETURN:
                     if (SDL_GetWindowFlags(window) & SDL_WINDOW_FULLSCREEN) {
                        SDL_SetWindowFullscreen(window, 0);
                     } else {
                        SDL_SetWindowFullscreen(window, SDL_WINDOW_FULLSCREEN);
                     }
                     break;
               }
            }
            break;

         case SDL_TEXTINPUT:
            if (strlen(event.text.text) == 1) {
               sma->key = toupper(event.text.text[0]) | 0x80;
            }
            break;
      }
   }
   return true;
}

static bool ewm_sma_step_cpu(struct ewm_sma_t *sma, int cycles) {
   while (true) {
      int ret = cpu_step(sma->cpu);
      if (ret < 0) {
         switch (ret) {
            case EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION:
               fprintf(stderr, "CPU: Exited because of unimplemented instructions 0x%.2x at 0x%.4x\n",
                       mem_get_byte(sma->cpu, sma->cpu->state.pc), sma->cpu->state.pc);
               break;
            case EWM_CPU_ERR_STACK_OVERFLOW:
               fprintf(stderr, "CPU: Exited because of stack overflow at 0x%.4x\n", sma->cpu->state.pc);
               break;
            case EWM_CPU_ERR_STACK_UNDERFLOW:
               fprintf(stderr, "CPU: Exited because of stack underflow at 0x%.4x\n", sma->cpu->state.pc);
               break;
         }
         cpu_nmi(sma->cpu);
      }
      cycles -= ret;
      if (cycles <= 0) {
         break;
      }
   }
   return true;
}

static uint8_t ewm_sma_tty_buffer_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct ewm_sma_t *sma = (struct ewm_sma_t*) mem->obj;
   return sma->tty->screen_buffer[addr - mem->start];
}

static void ewm_sma_tty_buffer_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct ewm_sma_t *sma = (struct ewm_sma_t*) mem->obj;
   sma->tty->screen_buffer[addr - mem->start] = b;
   sma->tty->screen_dirty = true;
}

static struct cpu_vectors_t ewm_sma_cpu_vectors = { 0, 0, 0 };

static int ewm_sma_init(struct ewm_sma_t *sma, SDL_Renderer *renderer) {
   memset(sma, 0, sizeof(struct ewm_sma_t));

   sma->cpu = cpu_create(EWM_CPU_MODEL_6502);

   sma->ram = cpu_add_ram(sma->cpu, EWM_SMA_RAM_BASE, EWM_SMA_RAM_BASE + EWM_SMA_RAM_SIZE - 1);
   sma->rom = cpu_add_rom_file(sma->cpu, EWM_SMA_ROM_BASE, "rom/sma.bin");
   sma->rom = cpu_add_rom_data(sma->cpu, EWM_CPU_VECTORS_BASE, EWM_CPU_VECTORS_BASE + EWM_CPU_VECTORS_SIZE - 1,
                               (uint8_t*) &ewm_sma_cpu_vectors);
   sma->tty = ewm_tty_create(renderer);
   if (sma->tty == NULL) {
      fprintf(stderr, "[SMA] Could not create TTY\n");
      return -1;
   }
   sma->tty_iom = cpu_add_iom(sma->cpu, EWM_SMA_TTY_BASE, EWM_SMA_TTY_BASE + EWM_SMA_TTY_SIZE - 1,
                              sma->tty, ewm_sma_tty_buffer_read, ewm_sma_tty_buffer_write);


   return 0;
}

static struct ewm_sma_t *ewm_sma_create(SDL_Renderer *renderer) {
   struct ewm_sma_t *sma = malloc(sizeof(struct ewm_sma_t));
   if (ewm_sma_init(sma, renderer) != 0) {
      free(sma);
      sma = NULL;
   }
   return sma;
}

int ewm_sma_main(int argc, char **argv) {
   // Initialize SDL

   if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS | SDL_INIT_GAMECONTROLLER) < 0) {
      fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
      exit(1);
   }

   SDL_Window *window = SDL_CreateWindow("EWM v0.1 / System Management Agent", 400, 60, 280*3, 192*3, SDL_WINDOW_SHOWN);
   if (window == NULL) {
      fprintf(stderr, "Failed create window: %s\n", SDL_GetError());
      exit(1);
   }

   SDL_Renderer *renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);
   if (renderer == NULL) {
      fprintf(stderr, "Failed to create renderer: %s\n", SDL_GetError());
      exit(1);
   }

   SDL_RenderSetLogicalSize(renderer, 280*3, 192*3);

   // Create the System Management Agent

   struct ewm_sma_t *sma = ewm_sma_create(renderer);
   cpu_reset(sma->cpu);

   //

   SDL_StartTextInput();

   Uint32 ticks = SDL_GetTicks();

   while (true) {
      if (!ewm_sma_poll_event(sma, window)) {
         break;
      }

      if ((SDL_GetTicks() - ticks) >= (1000 / 50)) {
         if (!ewm_sma_step_cpu(sma, 1000000 / 50)) {
            break;
         }

         if (sma->tty->screen_dirty) {
            ewm_tty_refresh(sma->tty);
            sma->tty->screen_dirty = false;
            SDL_RenderPresent(sma->tty->renderer);
         }

         ticks = SDL_GetTicks();
      }
   }

   // Destroy SDL

   SDL_DestroyRenderer(renderer);
   SDL_DestroyWindow(window);
   SDL_Quit();

   return 0;
}
