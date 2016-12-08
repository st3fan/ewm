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

#include <stdint.h>
#include <stdbool.h>

#include <SDL2/SDL.h>

#include "mem.h"
#include "cpu.h"
#include "a2p.h"
#include "chr.h"
#include "scr.h"

SDL_Window *window;
SDL_Renderer *renderer;

struct ewm_chr_t *chr = NULL;

void scr_init() {
  if (SDL_Init(SDL_INIT_VIDEO) < 0) {
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

  //

  chr = ewm_chr_create("roms/3410036.bin", EWM_CHR_ROM_TYPE_2716, renderer);
  if (chr == NULL) {
     fprintf(stderr, "[SCR] Failed to initialize character generator\n");
     exit(1);
  }
}

static int screen1_offsets[24] = {
   0x400, 0x480, 0x500, 0x580, 0x600, 0x680, 0x700, 0x780, 0x428, 0x4a8, 0x528, 0x5a8,
   0x628, 0x6a8, 0x728, 0x7a8, 0x450, 0x4d0, 0x550, 0x5d0, 0x650, 0x6d0, 0x750, 0x7d0
};

static int screen2_offsets[24] = {
   0x800, 0x880, 0x900, 0x980, 0xa00, 0xa80, 0xb00, 0xb80, 0x828, 0x8a8, 0x928, 0x9a8,
   0xa28, 0xaa8, 0xb28, 0xba8, 0x850, 0x8d0, 0x950, 0x9d0, 0xa50, 0xad0, 0xb50, 0xbd0
};

static inline void _render_character(struct a2p_t *a2p, int row, int column, int *offsets) {
   uint8_t c = a2p->screen_txt_data[(offsets[row] + column) - 0x0400];
   if (chr->characters[c] != NULL) {
      SDL_Rect dst;
      dst.x = column * 21;
      dst.y = row * 24;
      dst.w = 21;
      dst.h = 24;
      SDL_RenderCopy(renderer, chr->characters[c], NULL, &dst);
   }
}

static SDL_Color lores_colors[16] = {
   { 0,   0,   0,   0 }, // 0 Black
   { 255, 0,   255, 0 }, // 1 Magenta
   { 0,   0,   204, 0 }, // 2 Dark Blue
   { 128, 0,   128, 0 }, // 3 Purple
   { 0,   100, 0,   0 }, // 4 Dark Green
   { 128, 128, 128, 0 }, // 5 Grey 1
   { 0,   0,   205, 0 }, // 6 Medium Blue
   { 173, 216, 230, 0 }, // 7 Light Blue
   { 165, 42,  42,  0 }, // 8 Brown
   { 255, 165, 0,   0 }, // 9 Orange
   { 211, 211, 211, 0 }, // 10 Grey 2
   { 255, 192, 203, 0 }, // 11 Pink
   { 144, 238, 144, 0 }, // 12 Light Green
   { 255, 255, 0,   0 }, // 13 Yellow
   { 127, 255, 212, 0 }, // 14 Aquamarine
   { 255, 255, 255, 0 }, // 15 White
};

static inline void _render_lores_block(struct a2p_t *a2p, int row, int column, int *offsets) {
   uint8_t block = a2p->screen_txt_data[(offsets[row] + column) - 0x0400];
   if (block != 0) {
      SDL_Rect dst;
      dst.x = column * 21;
      dst.y = row * 24;
      dst.w = 21;
      dst.h = 12;

      uint c = block & 0x0f;
      if (c != 0) {
         SDL_SetRenderDrawColor(renderer, lores_colors[c].r, lores_colors[c].g, lores_colors[c].b, lores_colors[c].a);
         SDL_RenderFillRect(renderer, &dst);
      }

      c = (block & 0xf0) >> 4;
      if (c != 0) {
         dst.y += 12;
         SDL_SetRenderDrawColor(renderer, lores_colors[c].r, lores_colors[c].g, lores_colors[c].b, lores_colors[c].a);
         SDL_RenderFillRect(renderer, &dst);
      }
   }
}

static void _render_txt_screen1(struct a2p_t *a2p) {
   for (int row = 0; row < 24; row++) {
      for (int column = 0; column < 40; column++) {
         _render_character(a2p, row, column, screen1_offsets);
      }
   }
}

static void _render_txt_screen2(struct a2p_t *a2p) {
   for (int row = 0; row < 24; row++) {
      for (int column = 0; column < 40; column++) {
         _render_character(a2p, row, column, screen2_offsets);
      }
   }
}

static void _render_lgr_screen1(struct a2p_t *a2p, bool mixed) {
   // Render graphics
   int rows = mixed ? 20 : 24;
   for (int row = 0; row < rows; row++) {
      for (int column = 0; column < 40; column++) {
         _render_lores_block(a2p, row, column, screen1_offsets);
      }
   }
   // Render bottom 4 lines
   if (mixed) {
      for (int row = 20; row < 24; row++) {
         for (int column = 0; column < 40; column++) {
            _render_character(a2p, row, column, screen1_offsets);
         }
      }
   }
}

static void _render_lgr_screen2(struct a2p_t *a2p, bool mixed) {
   // Render graphics
   int rows = mixed ? 20 : 24;
   for (int row = 0; row < rows; row++) {
      for (int column = 0; column < 40; column++) {
         _render_lores_block(a2p, row, column, screen2_offsets);
      }
   }
   // Render bottom 4 lines
   if (mixed) {
      for (int row = 20; row < 24; row++) {
         for (int column = 0; column < 40; column++) {
            _render_character(a2p, row, column, screen2_offsets);
         }
      }
   }
}

static uint16_t hgr_page_offsets[2] = {
   0x0000, // $0000 in our buffer, $2000 in emulator
   0x2000  // $2000 in our buffer, $4000 in emulator
};

static uint16_t hgr_line_offsets[192] = {
   0x0000, 0x0400, 0x0800, 0x0c00, 0x1000, 0x1400, 0x1800, 0x1c00,
   0x0080, 0x0480, 0x0880, 0x0c80, 0x1080, 0x1480, 0x1880, 0x1c80,
   0x0100, 0x0500, 0x0900, 0x0d00, 0x1100, 0x1500, 0x1900, 0x1d00,
   0x0180, 0x0580, 0x0980, 0x0d80, 0x1180, 0x1580, 0x1980, 0x1d80,
   0x0200, 0x0600, 0x0a00, 0x0e00, 0x1200, 0x1600, 0x1a00, 0x1e00,
   0x0280, 0x0680, 0x0a80, 0x0e80, 0x1280, 0x1680, 0x1a80, 0x1e80,
   0x0300, 0x0700, 0x0b00, 0x0f00, 0x1300, 0x1700, 0x1b00, 0x1f00,
   0x0380, 0x0780, 0x0b80, 0x0f80, 0x1380, 0x1780, 0x1b80, 0x1f80,
   0x0028, 0x0428, 0x0828, 0x0c28, 0x1028, 0x1428, 0x1828, 0x1c28,
   0x00a8, 0x04a8, 0x08a8, 0x0ca8, 0x10a8, 0x14a8, 0x18a8, 0x1ca8,
   0x0128, 0x0528, 0x0928, 0x0d28, 0x1128, 0x1528, 0x1928, 0x1d28,
   0x01a8, 0x05a8, 0x09a8, 0x0da8, 0x11a8, 0x15a8, 0x19a8, 0x1da8,
   0x0228, 0x0628, 0x0a28, 0x0e28, 0x1228, 0x1628, 0x1a28, 0x1e28,
   0x02a8, 0x06a8, 0x0aa8, 0x0ea8, 0x12a8, 0x16a8, 0x1aa8, 0x1ea8,
   0x0328, 0x0728, 0x0b28, 0x0f28, 0x1328, 0x1728, 0x1b28, 0x1f28,
   0x03a8, 0x07a8, 0x0ba8, 0x0fa8, 0x13a8, 0x17a8, 0x1ba8, 0x1fa8,
   0x0050, 0x0450, 0x0850, 0x0c50, 0x1050, 0x1450, 0x1850, 0x1c50,
   0x00d0, 0x04d0, 0x08d0, 0x0cd0, 0x10d0, 0x14d0, 0x18d0, 0x1cd0,
   0x0150, 0x0550, 0x0950, 0x0d50, 0x1150, 0x1550, 0x1950, 0x1d50,
   0x01d0, 0x05d0, 0x09d0, 0x0dd0, 0x11d0, 0x15d0, 0x19d0, 0x1dd0,
   0x0250, 0x0650, 0x0a50, 0x0e50, 0x1250, 0x1650, 0x1a50, 0x1e50,
   0x02d0, 0x06d0, 0x0ad0, 0x0ed0, 0x12d0, 0x16d0, 0x1ad0, 0x1ed0,
   0x0350, 0x0750, 0x0b50, 0x0f50, 0x1350, 0x1750, 0x1b50, 0x1f50,
   0x03d0, 0x07d0, 0x0bd0, 0x0fd0, 0x13d0, 0x17d0, 0x1bd0, 0x1fd0
};

static void _render_hgr_line(struct a2p_t *a2p, int line, uint16_t line_base) {
   int x = 0;
   for (int i = 0; i < 40; i++) {
      uint8_t c = a2p->screen_hgr_data[line_base + i];
      for (int j = 0; j < 7; j++) {
         SDL_Rect dst;
         dst.x = x * 3;
         dst.y = line * 3;
         dst.w = 3;
         dst.h = 3;
         if (c & (1 << j)) {
            SDL_SetRenderDrawColor(renderer, 0, 255, 0, 0);
         } else {
            SDL_SetRenderDrawColor(renderer, 0, 0, 0, 0);
         }
         SDL_RenderFillRect(renderer, &dst);
         x++;
      }
   }
}

static void _render_hgr_screen(struct a2p_t *a2p) {
   // Render graphics
   int lines = (a2p->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED) ? 168  : 192;
   uint16_t hgr_base = hgr_page_offsets[a2p->screen_page];
   for (int line = 0; line < lines; line++) {
      uint16_t line_base = hgr_base + hgr_line_offsets[line];
      _render_hgr_line(a2p, line, line_base);
   }

   // Render bottom 4 lines of text
   if (a2p->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED) {
      for (int row = 20; row < 24; row++) {
         for (int column = 0; column < 40; column++) {
            _render_character(a2p, row, column, screen1_offsets);
         }
      }
   }
}

void scr_main(struct cpu_t *cpu, struct a2p_t *a2p) {
  bool quit = false;
  //bool running = true;

  SDL_StartTextInput();

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
        }
     }

     // Logic

     for (int i = 0; i < 1000; i++) {
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

     // Render

     if (a2p->screen_dirty) {
        SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
        SDL_RenderClear(renderer);

        switch (a2p->screen_mode) {
           case EWM_A2P_SCREEN_MODE_TEXT:
              switch (a2p->screen_page) {
                 case EWM_A2P_SCREEN_PAGE1:
                    _render_txt_screen1(a2p);
                    break;
                 case EWM_A2P_SCREEN_PAGE2:
                    _render_txt_screen2(a2p);
                    break;
              }
              break;
           case EWM_A2P_SCREEN_MODE_GRAPHICS:
              switch (a2p->screen_graphics_mode) {
                 case EWM_A2P_SCREEN_GRAPHICS_MODE_LGR:
                    switch (a2p->screen_page) {
                       case EWM_A2P_SCREEN_PAGE1:
                          _render_lgr_screen1(a2p, a2p->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED);
                          break;
                       case EWM_A2P_SCREEN_PAGE2:
                          _render_lgr_screen2(a2p, a2p->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED);
                          break;
                    }
                    break;
                 case EWM_A2P_SCREEN_GRAPHICS_MODE_HGR:
                    _render_hgr_screen(a2p);
                    break;
              }
              break;
        }

        SDL_RenderPresent(renderer);

        a2p->screen_dirty = false;
     }
  }

  SDL_DestroyWindow(window);
  SDL_Quit();
}
