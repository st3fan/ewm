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

#include <ctype.h>
#include <getopt.h>
#include <stdlib.h>

#include <SDL2/SDL.h>

#include "sdl.h"
#include "cpu.h"
#include "mem.h"
#include "pia.h"
#include "tty.h"
#include "dbg.h"
#include "one.h"

static void ewm_one_pia_callback(struct ewm_pia_t *pia, void *obj, uint8_t ddr, uint8_t v) {
   struct ewm_one_t *one = (struct ewm_one_t*) obj;
   if (one->model == EWM_ONE_MODEL_APPLE1) {
      v &= 0x7f;
   }
   ewm_tty_write(one->tty, v);
}

static int ewm_one_init(struct ewm_one_t *one, int model, SDL_Renderer *renderer) {
   memset(one, 0, sizeof(struct ewm_one_t));
   one->model = model;
   switch (model) {
      case EWM_ONE_MODEL_APPLE1:
         one->cpu = cpu_create(EWM_CPU_MODEL_6502);
         cpu_add_ram(one->cpu, 0x0000, 8 * 1024 - 1);
         cpu_add_rom_file(one->cpu, 0xff00, "rom/apple1.rom");
         one->tty = ewm_tty_create(renderer);
         one->pia = ewm_pia_create(one->cpu);
         one->pia->callback = ewm_one_pia_callback;
         one->pia->callback_obj = one;
         break;
      case EWM_ONE_MODEL_REPLICA1:
         one->cpu = cpu_create(EWM_CPU_MODEL_65C02);
         cpu_add_ram(one->cpu, 0x0000, 32 * 1024 - 1);
         cpu_add_rom_file(one->cpu, 0xe000, "rom/krusader.rom");
         one->tty = ewm_tty_create(renderer);
         one->pia = ewm_pia_create(one->cpu);
         one->pia->callback = ewm_one_pia_callback;
         one->pia->callback_obj = one;
         break;
   }
   return 0;
}

void ewm_one_destroy(struct ewm_one_t *one) {
   // TODO
}

static void ewm_one_keydown(struct ewm_one_t *one, uint8_t key) {
   ewm_pia_set_ina(one->pia, key | 0x80);
   ewm_pia_set_irqa1(one->pia);
}

static bool ewm_one_poll_event(struct ewm_one_t *one, SDL_Window *window) {
   SDL_Event event;
   while (SDL_PollEvent(&event) != 0) {
      switch (event.type) {
         case SDL_QUIT:
            return false;
         case SDL_WINDOWEVENT:
            one->tty->screen_dirty = true;
            break;
         case SDL_KEYDOWN:
            if (event.key.keysym.mod & KMOD_CTRL) {
               if (event.key.keysym.sym >= SDLK_a && event.key.keysym.sym <= SDLK_z) {
                  ewm_one_keydown(one, event.key.keysym.sym - SDLK_a);
               } else {
                  // TODO Implement control codes 1b - 1f
               }
            } else if (event.key.keysym.mod & KMOD_GUI) {
               switch (event.key.keysym.sym) {
                  case SDLK_ESCAPE:
                     cpu_reset(one->cpu);
                     break;
                  case SDLK_RETURN:
                     if (SDL_GetWindowFlags(window) & SDL_WINDOW_FULLSCREEN) {
                        SDL_SetWindowFullscreen(window, 0);
                     } else {
                        SDL_SetWindowFullscreen(window, SDL_WINDOW_FULLSCREEN);
                     }
                     break;
               }
            } else if (event.key.keysym.mod == KMOD_NONE) {
               switch (event.key.keysym.sym) {
                  case SDLK_RETURN:
                     ewm_one_keydown(one, 0x0d); // CR
                     break;
                  case SDLK_TAB:
                     ewm_one_keydown(one, 0x09); // HT
                  case SDLK_DELETE:
                     ewm_one_keydown(one, 0x7f); // DEL
                     break;
                  case SDLK_LEFT:
                     ewm_one_keydown(one, 0x08); // BS
                     break;
                  case SDLK_RIGHT:
                     ewm_one_keydown(one, 0x15); // NAK
                     break;
                  case SDLK_UP:
                     ewm_one_keydown(one, 0x0b); // VT
                     break;
                  case SDLK_DOWN:
                     ewm_one_keydown(one, 0x0a); // LF
                     break;
                  case SDLK_ESCAPE:
                     ewm_one_keydown(one, 0x1b); // ESC
                     break;
               }
            }
            break;
         case SDL_TEXTINPUT:
            if (strlen(event.text.text) == 1) {
               ewm_one_keydown(one, toupper(event.text.text[0]));
            }
            break;
      }
   }
   return true;
}

static bool ewm_one_step_cpu(struct ewm_one_t *one, int cycles) {
   if (one->cpu->status == EWM_CPU_STATUS_PAUSED) {
      return true;
   }

   while (true) {
      int ret = cpu_step(one->cpu);
      if (ret < 0) {
         // These only happen in strict mode
         switch (ret) {
            case EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION:
               fprintf(stderr, "CPU: Exited because of unimplemented instructions 0x%.2x at 0x%.4x\n",
                       mem_get_byte(one->cpu, one->cpu->state.pc), one->cpu->state.pc);
               break;
            case EWM_CPU_ERR_STACK_OVERFLOW:
               fprintf(stderr, "CPU: Exited because of stack overflow at 0x%.4x\n", one->cpu->state.pc);
               break;
            case EWM_CPU_ERR_STACK_UNDERFLOW:
               fprintf(stderr, "CPU: Exited because of stack underflow at 0x%.4x\n", one->cpu->state.pc);
               break;
         }
         return false;
      }

      cycles -= ret;
      if (cycles <= 0) {
         break;
      }
   }
   return true;
}

#define EWM_ONE_OPT_HELP   (0)
#define EWM_ONE_OPT_MODEL  (1)
#define EWM_ONE_OPT_MEMORY (2)
#define EWM_ONE_OPT_TRACE  (3)
#define EWM_ONE_OPT_STRICT (4)
#define EWM_ONE_OPT_DEBUG  (5)

static struct option one_options[] = {
   { "help",   no_argument,       NULL, EWM_ONE_OPT_HELP   },
   { "model",  required_argument, NULL, EWM_ONE_OPT_MODEL  },
   { "memory", required_argument, NULL, EWM_ONE_OPT_MEMORY },
   { "trace",  optional_argument, NULL, EWM_ONE_OPT_TRACE  },
   { "strict", no_argument,       NULL, EWM_ONE_OPT_STRICT },
   { "debug",  no_argument,       NULL, EWM_ONE_OPT_DEBUG  },
   { NULL,     0,                 NULL, 0 }
};

static void usage() {
   fprintf(stderr, "Usage: ewm one [options]\n");
   fprintf(stderr, "  --model <model>   model to emulate (default: apple1)\n");
   fprintf(stderr, "  --memory <region> add memory region (ram|rom:address:path)\n");
   fprintf(stderr, "  --trace <file>    trace cpu to file\n");
   fprintf(stderr, "  --strict          run emulator in strict mode\n");
   fprintf(stderr, "  --debug <port>    run debugger on port (default: 6502)\n");
   fprintf(stderr, "\n");
   fprintf(stderr, "Supported models:\n");
   fprintf(stderr, "  apple1    Classic Apple 1, 6502, 8KB RAM, Woz Monitor\n");
   fprintf(stderr, "  replica1  Replica 1, 65C02, 48KB RAM, KRUSADER\n");
}

int ewm_one_main(int argc, char **argv) {
   // Parse Apple 1 specific options
   int model = EWM_ONE_MODEL_DEFAULT;
   struct ewm_memory_option_t *extra_memory = NULL;
   char *trace_path = NULL;
   bool strict = false;
   int debug_port = 0;

   int ch;
   while ((ch = getopt_long_only(argc, argv, "", one_options, NULL)) != -1) {
      switch (ch) {
         case EWM_ONE_OPT_HELP: {
            usage();
            exit(0);
         }
         case EWM_ONE_OPT_MODEL: {
            if (strcmp(optarg, "apple1") == 0) {
               model = EWM_ONE_MODEL_APPLE1;
            } else if (strcmp(optarg, "replica1") == 0) {
               model = EWM_ONE_MODEL_REPLICA1;
            } else {
               fprintf(stderr, "Unknown --model specified\n");
               exit(1);
            }
            break;
         }
         case EWM_ONE_OPT_MEMORY: {
            struct ewm_memory_option_t *m = parse_memory_option(optarg);
            if (m == NULL) {
               exit(1);
            }
            m->next = extra_memory;
            extra_memory = m;
            break;
         }
         case EWM_ONE_OPT_TRACE: {
            trace_path = optarg ? optarg : "/dev/stderr";
            break;
         }
         case EWM_ONE_OPT_STRICT: {
            strict = true;
            break;
         }
         case EWM_ONE_OPT_DEBUG: {
            debug_port = optarg ? atoi(optarg) : 6502;
            if (debug_port == 0) {
               fprintf(stderr, "Invalid debugger port\n");
               exit(1);
            }
            break;
         }
         default: {
            usage();
            exit(1);
         }
      }
   }

   // Setup SDL

   if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS) < 0) {
      fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Window *window = SDL_CreateWindow("EWM v0.1 - Apple 1", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
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

   // Create the machine

   struct ewm_one_t *one = ewm_one_create(model, renderer);
   if (one == NULL) {
      fprintf(stderr, "Failed to create ewm_one_t\n");
      return 1;
   }

   // Add extra memory, if any

   if (extra_memory != NULL) {
      if (cpu_add_memory_from_options(one->cpu, extra_memory) != 0) {
         exit(1);
      }
   }

   cpu_strict(one->cpu, strict);
   cpu_trace(one->cpu, trace_path);
   cpu_reset(one->cpu);

   if (debug_port != 0) {
      struct ewm_dbg_t *dbg = ewm_dbg_create(one->cpu);
      ewm_dbg_pause(dbg);
      if (ewm_dbg_start(dbg) != 0) {
         printf("ewm: one: failed to start debugger\n");
         exit(1);
      }
   }

   // Main loop

   SDL_StartTextInput();

   uint32_t ticks = SDL_GetTicks();
   uint32_t phase = 1;

   while (true) {
      if (!ewm_one_poll_event(one, window)) { // TODO Move window into one
         break;
      }

      // This is very basic throttling that does bursts of CPU cycles.

      if ((SDL_GetTicks() - ticks) >= (1000 / EWM_ONE_FPS)) {
         if (!ewm_one_step_cpu(one, EWM_ONE_CPS / EWM_ONE_FPS)) {
            break;
         }

         if (one->tty->screen_dirty || (phase == 0) || ((phase % (EWM_ONE_FPS / 4)) == 0)) {
            SDL_SetRenderDrawColor(one->tty->renderer, 0, 0, 0, 255);
            SDL_RenderClear(one->tty->renderer);

            ewm_tty_refresh(one->tty, phase, EWM_ONE_FPS);
            one->tty->screen_dirty = false;

            SDL_Texture *texture = SDL_CreateTextureFromSurface(one->tty->renderer, one->tty->surface);
            if (texture != NULL) {
               SDL_RenderCopy(one->tty->renderer, texture, NULL, NULL);
               SDL_DestroyTexture(texture);
            }

            SDL_RenderPresent(one->tty->renderer);
         }

         ticks = SDL_GetTicks();

         phase += 1;
         if (phase == EWM_ONE_FPS) {
            phase = 0;
         }
      }
   }

   // Destroy SDL

   SDL_DestroyWindow(window);
   SDL_DestroyRenderer(renderer);
   SDL_Quit();

   return 0;
}

struct ewm_one_t *ewm_one_create(int model, SDL_Renderer *renderer) {
   struct ewm_one_t *one = (struct ewm_one_t*) malloc(sizeof(struct ewm_one_t));
   if (ewm_one_init(one, model, renderer) != 0) {
      free(one);
      one = NULL;
   }
   return one;
}
