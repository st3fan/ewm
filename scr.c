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

#include <SDL2/SDL_ttf.h>
#include <SDL2/SDL.h>

#include "mem.h"
#include "cpu.h"
#include "a2p.h"
#include "scr.h"

SDL_Texture *prerender_character(SDL_Renderer *renderer, TTF_Font *font, char c, bool inverse) {
  char text[2];
  text[0] = c;
  text[1] = 0x00;

  SDL_Color white;
  white.a = 0;
  white.r = 255;
  white.g = 255;
  white.b = 255;

  SDL_Color black;
  black.a = 0;
  black.r = 0;
  black.g = 0;
  black.b = 0;

  SDL_Surface *surface;
  if (inverse) {
     surface = TTF_RenderText_Shaded(font, text, black, white);
  } else {
     surface = TTF_RenderText_Shaded(font, text, white, black);
  }
  SDL_Texture *texture = SDL_CreateTextureFromSurface(renderer, surface);

  SDL_FreeSurface(surface);

  return texture;
}

SDL_Texture *characters[256];

void render_character(SDL_Texture *texture, SDL_Renderer *renderer, int x, int y) {
   SDL_Rect dst;
   dst.x = x * 21;
   dst.y = y * 24;
   dst.w = 21;
   dst.h = 24;
   SDL_RenderCopy(renderer, texture, NULL, &dst);
}

SDL_Window *window;
SDL_Renderer *renderer;

void scr_init() {
  if (SDL_Init(SDL_INIT_VIDEO) < 0) {
    fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
    exit(1);
  }

  //

  window = SDL_CreateWindow("Test", 40, 60, 280*3, 192*3, SDL_WINDOW_SHOWN);
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

  TTF_Init();
  TTF_Font *font = TTF_OpenFont("Apple2Forever.ttf", 48);

  memset(characters, 0x00, sizeof(characters));

  // normal text
  for (int c = 0x20; c <= 0x60; c++) {
    SDL_Texture *texture = prerender_character(renderer, font, (char) c, false);
    if (texture == NULL) {
       fprintf(stderr, "Failed to create character texture (%.2x): %s\n", c, SDL_GetError());
      exit(1);
    }
    characters[0xa0 + (c - 0x20)] = texture;
  }

  // inverse text
  for (int c = 0x40; c <= 0x60; c++) {
    SDL_Texture *texture = prerender_character(renderer, font, (char) c, true);
    if (texture == NULL) {
      fprintf(stderr, "Failed to create character texture: %s\n", SDL_GetError());
      exit(1);
    }
    characters[0x00 + (c - 0x40)] = texture;
  }
  for (int c = 0x20; c <= 0x3f; c++) {
    SDL_Texture *texture = prerender_character(renderer, font, (char) c, true);
    if (texture == NULL) {
      fprintf(stderr, "Failed to create character texture: %s\n", SDL_GetError());
      exit(1);
    }
    characters[0x20 + (c - 0x20)] = texture;
  }

  //

  /* SDL_Surface *surface = SDL_GetWindowSurface(window); */
  /* if (surface == NULL) { */
  /*    fprintf(stderr, "Could not get window surface: %s\n", SDL_GetError()); */
  /*    exit(1); */
  /* } */

  /* SDL_FillRect(surface, NULL, SDL_MapRGB(surface->format, 0x00, 0x00, 0x00)); */
  /* SDL_UpdateWindowSurface(window); */
}

static int screen1_offsets[24] = {
   0x400, 0x480, 0x500, 0x580, 0x600, 0x680, 0x700, 0x780, 0x428, 0x4a8, 0x528, 0x5a8,
   0x628, 0x6a8, 0x728, 0x7a8, 0x450, 0x4d0, 0x550, 0x5d0, 0x650, 0x6d0, 0x750, 0x7d0
};

static int screen2_offsets[24] = {
   0x800, 0x880, 0x900, 0x980, 0xa00, 0xa80, 0xb00, 0xb80, 0x828, 0x8a8, 0x928, 0x9a8,
   0xa28, 0xaa8, 0xb28, 0xba8, 0x850, 0x8d0, 0x950, 0x9d0, 0xa50, 0xad0, 0xb50, 0xbd0
};

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

     if (a2p->screen1_dirty || a2p->screen2_dirty) {
        SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
        SDL_RenderClear(renderer);

        switch (a2p->current_screen) {
           case 0:
              if (a2p->screen1_dirty) {
                 for (int row = 0; row < 24; row++) {
                    uint16_t row_offset = screen1_offsets[row] - 0x0400;
                    for (int column = 0; column < 40; column++) {
                       uint8_t c = a2p->screen1_data[row_offset + column];
                       render_character(characters[c], renderer, column, row);
                    }
                 }
              }
              break;
           case 1:
              if (a2p->screen2_dirty) {
                 for (int row = 0; row < 24; row++) {
                    uint16_t row_offset = screen2_offsets[row] - 0x0800;
                    for (int column = 0; column < 40; column++) {
                       uint8_t c = a2p->screen2_data[row_offset + column];
                       render_character(characters[c], renderer, column, row);
                    }
                 }
              }
              break;
        }

        a2p->screen1_dirty = false;
        a2p->screen2_dirty = false;

        SDL_RenderPresent(renderer);
     }
  }

  SDL_DestroyWindow(window);
  SDL_Quit();
}
