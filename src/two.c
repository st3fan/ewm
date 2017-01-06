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
#include <string.h>
#include <getopt.h>
#include <time.h>

#include <SDL2/SDL.h>

#include "cpu.h"
#include "mem.h"
#include "dsk.h"
#include "alc.h"
#include "chr.h"
#include "scr.h"
#include "two.h"


#define EWM_A2P_SS_KBD                  0xc000
#define EWM_A2P_SS_KBDSTRB              0xc010
#define EWM_A2P_SS_SPKR                 0xc030

#define EWM_A2P_SS_SCREEN_MODE_GRAPHICS 0xc050
#define EWM_A2P_SS_SCREEN_MODE_TEXT     0xc051
#define EWM_A2P_SS_GRAPHICS_STYLE_FULL  0xc052
#define EWM_A2P_SS_GRAPHICS_STYLE_MIXED 0xc053
#define EWM_A2P_SS_SCREEN_PAGE1         0xc054
#define EWM_A2P_SS_SCREEN_PAGE2         0xc055
#define EWM_A2P_SS_GRAPHICS_MODE_LGR    0xc056
#define EWM_A2P_SS_GRAPHICS_MODE_HGR    0xc057

#define EWM_A2P_SS_SETAN0  0xc058
#define EWM_A2P_SS_CLRAN0  0xc059
#define EWM_A2P_SS_SETAN1  0xc05a
#define EWM_A2P_SS_CLRAN1  0xc05b
#define EWM_A2P_SS_SETAN2  0xc05c
#define EWM_A2P_SS_CLRAN2  0xc05d
#define EWM_A2P_SS_SETAN3  0xc05e
#define EWM_A2P_SS_CLRAN3  0xc05f

#define EWM_A2P_SS_PB0 0xC061
#define EWM_A2P_SS_PB1 0xC062
#define EWM_A2P_SS_PB2 0xC063
#define EWM_A2P_SS_PB3 0xC060 // TODO On the gs only?

#define EWM_TWO_SS_PTRIG 0xc070
#define EWM_TWO_SS_PADL0 0xc064
#define EWM_TWO_SS_PADL1 0xc065
#define EWM_TWO_SS_PADL3 0xc066
#define EWM_TWO_SS_PADL4 0xc067


static uint8_t ewm_two_iom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct ewm_two_t *two = (struct ewm_two_t*) mem->obj;
   switch (addr) {
      case EWM_A2P_SS_KBD:
         return two->key;
      case EWM_A2P_SS_KBDSTRB:
         two->key &= 0x7f;
         return 0x00;

      case EWM_A2P_SS_SCREEN_MODE_GRAPHICS:
         two->screen_mode = EWM_A2P_SCREEN_MODE_GRAPHICS;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_MODE_TEXT:
         two->screen_mode = EWM_A2P_SCREEN_MODE_TEXT;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_MODE_LGR:
         two->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_LGR;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_MODE_HGR:
         two->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_HGR;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_STYLE_FULL:
         two->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_STYLE_MIXED:
         two->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_SCREEN_PAGE1:
         two->screen_page = EWM_A2P_SCREEN_PAGE1;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_PAGE2:
         two->screen_page = EWM_A2P_SCREEN_PAGE2;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_SPKR:
         // TODO Implement speaker support
         break;

      case EWM_A2P_SS_PB0:
         return two->buttons[0];
      case EWM_A2P_SS_PB1:
         return two->buttons[1];
      case EWM_A2P_SS_PB2:
         return two->buttons[2];
      case EWM_A2P_SS_PB3:
         return two->buttons[3];

      case EWM_A2P_SS_SETAN0:
         break;
      case EWM_A2P_SS_SETAN1:
         break;
      case EWM_A2P_SS_SETAN2:
         break;
      case EWM_A2P_SS_SETAN3:
         break;

      case EWM_A2P_SS_CLRAN0:
         break;
      case EWM_A2P_SS_CLRAN1:
         break;
      case EWM_A2P_SS_CLRAN2:
         break;
      case EWM_A2P_SS_CLRAN3:
         break;

      case EWM_TWO_SS_PTRIG: {
         if (two->joystick != NULL) {
            int x = 128 + (SDL_JoystickGetAxis(two->joystick, 0) / 256);
            two->padl0_time = two->cpu->counter + (x * (2820 / 255)); // TODO Remove magic values
            two->padl0_value = 0xff;
            int y = 128 + (SDL_JoystickGetAxis(two->joystick, 1) / 256);
            two->padl1_time = two->cpu->counter + (y * (2820 / 255)); // TODO Remove magic values
            two->padl1_value = 0xff;
         }
         break;
      }
      case EWM_TWO_SS_PADL0: {
         if (two->padl0_time != 0 && two->cpu->counter >= two->padl0_time) {
            two->padl0_time = 0;
            two->padl0_value = 0;
         }
         return two->padl0_value;
      }
      case EWM_TWO_SS_PADL1: {
         if (two->padl1_time != 0 && two->cpu->counter >= two->padl1_time) {
            two->padl1_value = 0;
         }
         return two->padl1_value;
      }

      default:
         printf("[A2P] Unexpected read at $%.4X pc is $%.4X\n", addr, cpu->state.pc);
         break;
   }
   return 0;
}

static void ewm_two_iom_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct ewm_two_t *two = (struct ewm_two_t*) mem->obj;
   switch (addr) {

      case EWM_A2P_SS_KBDSTRB:
         two->key &= 0x7f;
         break;

      case EWM_A2P_SS_SPKR:
         // TODO Implement speaker support
         break;

      case EWM_A2P_SS_SCREEN_MODE_GRAPHICS:
         two->screen_mode = EWM_A2P_SCREEN_MODE_GRAPHICS;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_MODE_TEXT:
         two->screen_mode = EWM_A2P_SCREEN_MODE_TEXT;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_MODE_LGR:
         two->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_LGR;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_MODE_HGR:
         two->screen_graphics_mode = EWM_A2P_SCREEN_GRAPHICS_MODE_HGR;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_GRAPHICS_STYLE_FULL:
         two->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_FULL;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_GRAPHICS_STYLE_MIXED:
         two->screen_graphics_style = EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_SCREEN_PAGE1:
         two->screen_page = EWM_A2P_SCREEN_PAGE1;
         two->screen_dirty = true;
         break;
      case EWM_A2P_SS_SCREEN_PAGE2:
         two->screen_page = EWM_A2P_SCREEN_PAGE2;
         two->screen_dirty = true;
         break;

      case EWM_A2P_SS_SETAN0:
         break;
      case EWM_A2P_SS_SETAN1:
         break;
      case EWM_A2P_SS_SETAN2:
         break;
      case EWM_A2P_SS_SETAN3:
         break;

      case EWM_A2P_SS_CLRAN0:
         break;
      case EWM_A2P_SS_CLRAN1:
         break;
      case EWM_A2P_SS_CLRAN2:
         break;
      case EWM_A2P_SS_CLRAN3:
         break;

      default:
         printf("[A2P] Unexpected write at $%.4X pc is $%.4X\n", addr, cpu->state.pc);
         break;
   }
}

static uint8_t ewm_two_screen_txt_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct ewm_two_t *two = (struct ewm_two_t*) mem->obj;
   return two->screen_txt_data[addr - mem->start];
}

static void ewm_two_screen_txt_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct ewm_two_t *two = (struct ewm_two_t*) mem->obj;
   two->screen_txt_data[addr - mem->start] = b;
   two->screen_dirty = true;
}

static uint8_t ewm_two_screen_hgr_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct ewm_two_t *two = (struct ewm_two_t*) mem->obj;
   return two->screen_hgr_data[addr - mem->start];
}

static void ewm_two_screen_hgr_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct ewm_two_t *two = (struct ewm_two_t*) mem->obj;
   two->screen_hgr_data[addr - mem->start] = b;
   two->screen_dirty = true;
}

static int ewm_two_init(struct ewm_two_t *two, int type, SDL_Renderer *renderer, SDL_Joystick *joystick) {
   memset(two, 0, sizeof(struct ewm_two_t));

   two->type = type;

   switch (type) {
      case EWM_TWO_TYPE_APPLE2: {
         return -1; // TODO
         break;
      }

      // Apple ][+ / Apple Language Card / Disk II Card with 2 drives
      case EWM_TWO_TYPE_APPLE2PLUS: {
         two->cpu = cpu_create(EWM_CPU_MODEL_6502);

         two->ram = cpu_add_ram(two->cpu, 0x0000, 48 * 1024);
         two->roms[0] = cpu_add_rom_file(two->cpu, 0xd000, "rom/341-0011.bin"); // AppleSoft BASIC D000
         two->roms[1] = cpu_add_rom_file(two->cpu, 0xd800, "rom/341-0012.bin"); // AppleSoft BASIC D800
         two->roms[2] = cpu_add_rom_file(two->cpu, 0xe000, "rom/341-0013.bin"); // AppleSoft BASIC E000
         two->roms[3] = cpu_add_rom_file(two->cpu, 0xe800, "rom/341-0014.bin"); // AppleSoft BASIC E800
         two->roms[4] = cpu_add_rom_file(two->cpu, 0xf000, "rom/341-0015.bin"); // AppleSoft BASIC F000
         two->roms[5] = cpu_add_rom_file(two->cpu, 0xf800, "rom/341-0020.bin"); // Autostart Monitor F800
         two->iom = cpu_add_iom(two->cpu, 0xc000, 0xc07f, two, ewm_two_iom_read, ewm_two_iom_write);

         two->dsk = ewm_dsk_create(two->cpu);
         if (two->dsk == NULL) {
            fprintf(stderr, "[TWO] Could not create Apple Disk Controller\n");
            return -1;
         }

         two->alc = ewm_alc_create(two->cpu);
         if (two->alc == NULL) {
            fprintf(stderr, "[TWO] Could not create Apple Language Card\n");
            return -1;
         }

         two->scr = ewm_scr_create(two, renderer);
         if (two->scr == NULL) {
            fprintf(stderr, "[TWO] Could not create Screen\n");
            return -1;
         }

         break;
      }

      case EWM_TWO_TYPE_APPLE2E: {
         return -1; // TODO
         break;
      }
   }

   // TODO Introduce ewm_scr_t that captures everything related to the apple 2 screen so that it can be re-used?

   two->screen_txt_data = malloc(2 * 1024);
   two->screen_txt_iom = cpu_add_iom(two->cpu, 0x0400, 0x0bff, two, ewm_two_screen_txt_read, ewm_two_screen_txt_write);

   two->screen_hgr_data = malloc(16 * 1024);
   two->screen_hgr_iom = cpu_add_iom(two->cpu, 0x2000, 0x5fff, two, ewm_two_screen_hgr_read, ewm_two_screen_hgr_write);

   two->joystick = joystick;

   return 0;
}

struct ewm_two_t *ewm_two_create(int type, SDL_Renderer *renderer, SDL_Joystick *joystick) {
   struct ewm_two_t *two = malloc(sizeof(struct ewm_two_t));
   if (ewm_two_init(two, type, renderer, joystick) != 0) {
      free(two);
      two = NULL;
   }
   return two;
}

void ewm_two_destroy(struct ewm_two_t *two) {
   // TODO
}

int ewm_two_load_disk(struct ewm_two_t *two, int drive, char *path) {
   return ewm_dsk_set_disk_file(two->dsk, drive, false, path);
}



static bool ewm_two_poll_event(struct ewm_two_t *two, SDL_Window *window) { // TODO Should window be part of ewm_two_t?
   SDL_Event event;
   while (SDL_PollEvent(&event) != 0) {
      switch (event.type) {
         case SDL_QUIT:
            return false;

         case SDL_WINDOWEVENT:
            two->screen_dirty = true;
            break;

         case SDL_CONTROLLERBUTTONDOWN:
         case SDL_CONTROLLERBUTTONUP:
            switch (event.cbutton.button) {
               case SDL_CONTROLLER_BUTTON_A:
               case SDL_CONTROLLER_BUTTON_LEFTSHOULDER:
                  two->buttons[0] = event.cbutton.state == SDL_PRESSED ? 0x80 : 0x00;
                  break;
               case SDL_CONTROLLER_BUTTON_B:
               case SDL_CONTROLLER_BUTTON_RIGHTSHOULDER:
                  two->buttons[1] = event.cbutton.state == SDL_PRESSED ? 0x80 : 0x00;
                  break;
               case SDL_CONTROLLER_BUTTON_X:
                  two->buttons[2] = event.cbutton.state == SDL_PRESSED ? 0x80 : 0x00;
                  break;
               case SDL_CONTROLLER_BUTTON_Y:
                  two->buttons[3] = event.cbutton.state == SDL_PRESSED ? 0x80 : 0x00;
                  break;
            }
            break;

         case SDL_KEYDOWN:
            if (event.key.keysym.mod & KMOD_CTRL) {
               if (event.key.keysym.sym >= SDLK_a && event.key.keysym.sym <= SDLK_z) {
                  two->key = (event.key.keysym.sym - SDLK_a) | 0x80;
               } else {
                  // TODO Implement control codes 1b - 1f
               }
            } else if (event.key.keysym.mod & KMOD_GUI) {
               switch (event.key.keysym.sym) {
                  case SDLK_ESCAPE:
                     fprintf(stderr, "[SDL] Reset\n");
                     cpu_reset(two->cpu);
                     break;
                  case SDLK_RETURN:
                     if (SDL_GetWindowFlags(window) & SDL_WINDOW_FULLSCREEN) {
                        SDL_SetWindowFullscreen(window, 0);
                     } else {
                        SDL_SetWindowFullscreen(window, SDL_WINDOW_FULLSCREEN);
                     }
                     break;
                  case SDLK_i:
                     two->status_bar_visible = !two->status_bar_visible;
                     SDL_SetWindowSize(window, 40*7*3, 24*8*3 + (two->status_bar_visible ? (9*3) : 0));
                     SDL_RenderSetLogicalSize(two->scr->renderer, 40*7*3, 24*8*3 + (two->status_bar_visible ? (9*3) : 0));
                     break;
               }
            } else if (event.key.keysym.mod == KMOD_NONE) {
               switch (event.key.keysym.sym) {
                  case SDLK_RETURN:
                     two->key = 0x0d | 0x80; // CR
                     break;
                  case SDLK_TAB:
                     two->key = 0x09 | 0x80; // HT
                  case SDLK_DELETE:
                     two->key = 0x7f | 0x80; // DEL
                     break;
                  case SDLK_BACKSPACE:
                  case SDLK_LEFT:
                     two->key = 0x08 | 0x80; // BS
                     break;
                  case SDLK_RIGHT:
                     two->key = 0x15 | 0x80; // NAK
                     break;
                  case SDLK_UP:
                     two->key = 0x0b | 0x80; // VT
                     break;
                  case SDLK_DOWN:
                     two->key = 0x0a | 0x80; // LF
                     break;
                  case SDLK_ESCAPE:
                     two->key = 0x1b | 0x80; // ESC
                     break;
               }
            }
            break;

         case SDL_KEYUP:
            if (event.key.keysym.mod & KMOD_ALT) {
               switch (event.key.keysym.sym) {
                  case SDLK_1:
                     two->buttons[0] = 0;
                     break;
                  case SDLK_2:
                     two->buttons[1] = 0;
                     break;
                  case SDLK_3:
                     two->buttons[2] = 0;
                     break;
                  case SDLK_4:
                     two->buttons[3] = 0;
                     break;
               }
            }
            break;

         case SDL_TEXTINPUT:
            if (strlen(event.text.text) == 1) {
               two->key = toupper(event.text.text[0]) | 0x80;
            }
            break;
      }
   }

   return true;
}

static bool ewm_two_step_cpu(struct ewm_two_t *two, int cycles) {
   while (true) {
      int ret = cpu_step(two->cpu);
      if (ret < 0) {
         // These only happen in strict mode
         switch (ret) {
            case EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION:
               fprintf(stderr, "CPU: Exited because of unimplemented instructions 0x%.2x at 0x%.4x\n",
                       mem_get_byte(two->cpu, two->cpu->state.pc), two->cpu->state.pc);
               break;
            case EWM_CPU_ERR_STACK_OVERFLOW:
               fprintf(stderr, "CPU: Exited because of stack overflow at 0x%.4x\n", two->cpu->state.pc);
               break;
            case EWM_CPU_ERR_STACK_UNDERFLOW:
               fprintf(stderr, "CPU: Exited because of stack underflow at 0x%.4x\n", two->cpu->state.pc);
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

static void ewm_two_update_status_bar(struct ewm_two_t *two, double mhz) {

   SDL_Rect rect = { .x = 0, .y = (24*8*3), .w = (40*7*3), .h = (9*3) };
   SDL_SetRenderDrawColor(two->scr->renderer, 39, 39, 39, 0);
   SDL_RenderFillRect(two->scr->renderer, &rect);

   char s[41];
   snprintf(s, 41, "%1.3f MHZ                         [1][2]", mhz);
   //               1234567890123456789012345678901234567890

   for (int i = 0; i < 40; i++) {
      int c = s[i] + 0x80;
      if (two->scr->chr->characters[c] != NULL) {
         SDL_Rect dst;
         dst.x = i * 21;
         dst.y = 24 * 24 + 3;
         dst.w = 21;
         dst.h = 24;

         if (two->dsk->on && ((i == 35 && two->dsk->drive == EWM_DSK_DRIVE1) || (i == 38 && two->dsk->drive == EWM_DSK_DRIVE2))) {
            SDL_SetTextureColorMod(two->scr->chr->characters[c], 145, 193, 75);
         } else {
            SDL_SetTextureColorMod(two->scr->chr->characters[c], 255, 0, 0);
         }

         SDL_RenderCopy(two->scr->renderer, two->scr->chr->characters[c], NULL, &dst);
      }
   }
}

#define EWM_TWO_OPT_DRIVE1 (0)
#define EWM_TWO_OPT_DRIVE2 (1)
#define EWM_TWO_OPT_COLOR  (2)
#define EWM_TWO_OPT_FPS    (3)
#define EWM_TWO_OPT_MEMORY (4)
#define EWM_TWO_OPT_TRACE  (5)
#define EWM_TWO_OPT_STRICT (6)
#define EWM_TWO_OPT_DEBUG  (7)

static struct option one_options[] = {
   { "drive1",  required_argument, NULL, EWM_TWO_OPT_DRIVE1 },
   { "drive2",  required_argument, NULL, EWM_TWO_OPT_DRIVE2 },
   { "color",   no_argument,       NULL, EWM_TWO_OPT_COLOR  },
   { "fps",     required_argument, NULL, EWM_TWO_OPT_FPS    },
   { "memory",  required_argument, NULL, EWM_TWO_OPT_MEMORY },
   { "trace",   optional_argument, NULL, EWM_TWO_OPT_TRACE  },
   { "strict",  no_argument,       NULL, EWM_TWO_OPT_STRICT },
   { "debug",   no_argument,       NULL, EWM_TWO_OPT_DEBUG  },
   { NULL,      0,                 NULL, 0 }
};

int ewm_two_main(int argc, char **argv) {
   // Parse options

   char *drive1 = NULL;
   char *drive2 = NULL;
   bool color = false;
   uint32_t fps = EWM_TWO_FPS_DEFAULT;
   struct ewm_memory_option_t *extra_memory = NULL;
   char *trace_path = NULL;
   bool strict = false;
   bool debug = false;

   int ch;
   while ((ch = getopt_long_only(argc, argv, "", one_options, NULL)) != -1) {
      switch (ch) {
         case EWM_TWO_OPT_DRIVE1:
            drive1 = optarg;
            break;
         case EWM_TWO_OPT_DRIVE2:
            drive2 = optarg;
            break;
         case EWM_TWO_OPT_COLOR:
            color = true;
            break;
         case EWM_TWO_OPT_FPS:
            fps = atoi(optarg);
            break;
         case EWM_TWO_OPT_MEMORY: {
            struct ewm_memory_option_t *m = parse_memory_option(optarg);
            if (m == NULL) {
               exit(1);
            }
            m->next = extra_memory;
            extra_memory = m;
            break;
         }
         case EWM_TWO_OPT_TRACE:
            trace_path = optarg ? optarg : "/dev/stderr";
            break;
         case EWM_TWO_OPT_STRICT:
            strict = true;
            break;
         case EWM_TWO_OPT_DEBUG:
            debug = true;
            break;
      }
   }

   // Initialize SDL

   if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS | SDL_INIT_GAMECONTROLLER) < 0) {
      fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
      exit(1);
   }

   SDL_Window *window = SDL_CreateWindow("EWM v0.1 / Apple ][+", 400, 60, 280*3, 192*3, SDL_WINDOW_SHOWN);
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

   // Print what renderer we got

   if (debug) {
      SDL_RendererInfo info;
      if (SDL_GetRendererInfo(renderer, &info) != 0) {
         fprintf(stderr, "Failed to get renderer info: %s\n", SDL_GetError());
         exit(1);
      }
      char flags[1024] = { 0 };
      if (info.flags & SDL_RENDERER_SOFTWARE) {
         strncat(flags, "SOFTWARE", sizeof(flags) - strlen(flags) - 1);
      }
      if (info.flags & SDL_RENDERER_ACCELERATED) {
         if (flags[0] != 0x00) {
            strncat(flags, "|", sizeof(flags) - strlen(flags) - 1);
         }
         strncat(flags, "ACCELERATED", sizeof(flags) - strlen(flags) - 1);
      }
      if (info.flags & SDL_RENDERER_PRESENTVSYNC) {
         if (flags[0] != 0x00) {
            strncat(flags, "|", sizeof(flags) - strlen(flags) - 1);
         }
         strncat(flags, "PRESENTVSYNC", sizeof(flags) - strlen(flags) - 1);
      }
      if (info.flags & SDL_RENDERER_TARGETTEXTURE) {
         if (flags[0] != 0x00) {
            strncat(flags, "|", sizeof(flags) - strlen(flags) - 1);
         }
         strncat(flags, "TARGETTEXTURE", sizeof(flags) - strlen(flags) - 1);
      }
      fprintf(stderr, "[TWO] Renderer name=%s flags=%s max_texture_size=(%d,%d)\n",
              info.name, flags, info.max_texture_width, info.max_texture_height);
   }

   // If we have a joystick, open it

   SDL_GameController *controller = NULL;
   SDL_Joystick *joystick = NULL;

   if (SDL_NumJoysticks() != 0) {
      controller = SDL_GameControllerOpen(0);
      SDL_GameControllerEventState(SDL_ENABLE);
      joystick = SDL_GameControllerGetJoystick(controller);
   }

   // Create and configure the Apple II

   struct ewm_two_t *two = ewm_two_create(EWM_TWO_TYPE_APPLE2PLUS, renderer, joystick);
   two->debug = debug;

   if (color) {
      ewm_scr_set_color_scheme(two->scr, EWM_SCR_COLOR_SCHEME_COLOR);
   }

   if (drive1 != NULL) {
      if (ewm_two_load_disk(two, EWM_DSK_DRIVE1, drive1) != 0) {
         fprintf(stderr, "[A2P] Cannot load Drive 1 with %s\n", drive1);
         exit(1);
      }
   }

   if (drive2 != NULL) {
      if (ewm_two_load_disk(two, EWM_DSK_DRIVE2, drive2) != 0) {
         fprintf(stderr, "[A2P] Cannot load Drive 2 with %s\n", drive2);
         exit(1);
      }
   }

   // Add extra memory, if any

   if (extra_memory != NULL) {
      if (cpu_add_memory_from_options(two->cpu, extra_memory) != 0) {
         exit(1);
      }
   }

   cpu_strict(two->cpu, strict);
   cpu_trace(two->cpu, trace_path);

   cpu_reset(two->cpu);

   //

   SDL_StartTextInput();

   uint32_t ticks = SDL_GetTicks();
   uint32_t phase = 1;

   uint64_t counter = two->cpu->counter;
   double mhz = 1.0;

   while (true) {
      if (!ewm_two_poll_event(two, window)) {
         break;
      }

      if ((SDL_GetTicks() - ticks) >= (1000 / fps)) {
         if (!ewm_two_step_cpu(two, EWM_TWO_SPEED / fps)) {
            break;
         }

         // Update the screen when it is flagged dirty or if we enter
         // the second half of the frames we draw each second. The
         // latter because that is when we update flashing text.

         if (two->screen_dirty || (phase == 0) || (phase == (fps / 2))) {
            ewm_scr_update(two->scr, phase, fps);
            two->screen_dirty = false;

            if (two->status_bar_visible) {
               ewm_two_update_status_bar(two, mhz);
            }

            SDL_RenderPresent(two->scr->renderer);
         }

         ticks = SDL_GetTicks();
         phase += 1;
         if (phase == fps) {
            phase = 0;

            // Calculate the number of cycles we have done in the past
            // second. TODO This will always equal 1023000 - It needs
            // to be actual clock time based instead. Good for now,
            // but not ideal.
            mhz = (two->cpu->counter - counter) / 1000000.0;
            counter = two->cpu->counter;
         }
      }
   }

   //

   SDL_DestroyRenderer(renderer);
   SDL_DestroyWindow(window);
   SDL_Quit();

   return 0;
}
