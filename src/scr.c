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
#include "two.h"
#include "chr.h"
#include "scr.h"

inline static void _set_pixel(SDL_Surface * surface, int x, int y, Uint32 color) {
  // TODO This will break if pitch is not very predictable or weird
  uint32_t *pixel = (uint32_t*) ((uint8_t*) surface->pixels + (y * 280*3*sizeof(uint32_t)) + (x * sizeof(uint32_t)));
  *pixel = color;
}

// Text rendering

static int txt_line_offsets[24] = {
   0x000, 0x080, 0x100, 0x180, 0x200, 0x280, 0x300, 0x380, 0x028, 0x0a8, 0x128, 0x1a8,
   0x228, 0x2a8, 0x328, 0x3a8, 0x050, 0x0d0, 0x150, 0x1d0, 0x250, 0x2d0, 0x350, 0x3d0
};

static inline void scr_render_character(struct scr_t *scr, int row, int column, bool flash) {
   uint16_t base = (scr->two->screen_page == EWM_A2P_SCREEN_PAGE1) ? 0x0400 : 0x0800;
   uint8_t c = scr->two->screen_txt_data[((txt_line_offsets[row] + base) + column) - 0x0400];
   if (scr->chr->textures[c] != NULL) {
      SDL_Rect src;
      src.x = 0;
      src.y = 0;
      src.w = 21;
      src.h = 24;

      SDL_Rect dst;
      dst.x = column * 21;
      dst.y = row * 24;
      dst.w = 21;
      dst.h = 24;

      if (c >= 0x40 && c <= 0x7f) {
         if (flash) {
            c -= 0x40;
         } else {
            if (c <= 0x5f) {
               c += 0x80;
            } else {
               c += 0x40;
            }
         }
      }

      if (scr->color_scheme == EWM_SCR_COLOR_SCHEME_MONOCHROME) {
         SDL_SetSurfaceColorMod(scr->chr->surfaces[c], 47, 249, 64);
      } else {
         SDL_SetSurfaceColorMod(scr->chr->surfaces[c], 255, 255, 255);
      }

      SDL_SetSurfaceBlendMode(scr->chr->surfaces[c], SDL_BLENDMODE_NONE);

      SDL_BlitSurface(scr->chr->surfaces[c], &src, SDL_GetWindowSurface(scr->window), &dst);
   }
}

static inline void scr_render_txt_screen(struct scr_t *scr, bool flash) {
   for (int row = 0; row < 24; row++) {
      for (int column = 0; column < 40; column++) {
         scr_render_character(scr, row, column, flash);
      }
   }
}

// Lores Rendering

static SDL_Color lores_colors_color[16] = {
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

static SDL_Color lores_colors_green[16] = {
   {  0,   0,  0, 0 }, // 0 Black
   {  9,  87, 17, 0 }, // 1 Magenta
   {  2,  22,  3, 0 }, // 2 Dark Blue
   { 19, 126, 29, 0 }, // 3 Purple
   { 11,  90, 18, 0 }, // 4 Dark Green
   { 13, 101, 21, 0 }, // 5 Grey 1
   {  8,  74, 14, 0 }, // 6 Medium Blue
   { 28, 170, 40, 0 }, // 7 Light Blue
   { 15, 109, 22, 0 }, // 8 Brown
   { 24, 152, 35, 0 }, // 9 Orange
   { 33, 181, 45, 0 }, // 10 Grey 2
   { 34, 190, 47, 0 }, // 11 Pink
   { 23, 149, 34, 0 }, // 12 Light Green
   { 43, 227, 58, 0 }, // 13 Yellow
   { 34, 197, 49, 0 }, // 14 Aquamarine
   { 47, 249, 64, 0 }, // 15 White
};

static inline void scr_render_lgr_screen(struct scr_t *scr, bool flash) {
   bool mixed = (scr->two->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED);
   SDL_Surface *surface = SDL_GetWindowSurface(scr->window);
   uint32_t *colors = scr->color_scheme == EWM_SCR_COLOR_SCHEME_COLOR ? scr->lores_colors_color : scr->lores_colors_green;
   uint16_t base = (scr->two->screen_page == EWM_A2P_SCREEN_PAGE1) ? 0x0400 : 0x0800;
   
   // Render graphics
   int rows = mixed ? 20 : 24;
   for (int row = 0; row < rows; row++) {
      for (int column = 0; column < 40; column++) {
	
	uint8_t block = scr->two->screen_txt_data[((txt_line_offsets[row] + base) + column) - 0x0400];
	if (block != 0) {

	  SDL_Rect dst;
	  dst.x = column * 21;
	  dst.y = row * 24;
	  dst.w = 21;
	  dst.h = 12;

	  uint8_t c = block & 0x0f;
	  if (c != 0) {
	    SDL_FillRect(surface, &dst, colors[c]);
	  }

	  c = (block & 0xf0) >> 4;
	  if (c != 0) {
	    dst.y += 12;
	    SDL_FillRect(surface, &dst, colors[c]);
	  }
	  
#if 0
	  int px = column * 21, py = row * 24;

	  uint8_t c = block & 0x0f;
	  if (c != 0) {
	    for (int h = 0; h < 12; h++) {
	      for (int w = 0; w <21; w++) {
		_set_pixel(surface, px+w, py+h, colors[c]);
	      }
	    }
	  }

	  py += 12;
      
	  c = (block & 0xf0) >> 4;
	  if (c != 0) {
	    for (int h = 0; h < 12; h++) {
	      for (int w = 0; w <21; w++) {
		_set_pixel(surface, px+w, py+h, colors[c]);
	      }
	    }
	  }
#endif
	  
	}
	
      }
   }
   
   // Render bottom 4 lines
   if (mixed) {
      for (int row = 20; row < 24; row++) {
         for (int column = 0; column < 40; column++) {
            scr_render_character(scr, row, column, flash);
         }
      }
   }
}

// Hires rendering

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

// CBBBBBBB

static SDL_Color hires_colors[16] = {
   { 0,   0,   0,   0 }, // 0 Black
   { 0,   149, 255, 0 }, // 1 Blue
   { 255, 64,  255, 0 }, // 2 Purple
   { 0,   249, 0,   0 }, // 3 Green
   { 255, 147, 0,   0 }, // 4 Red
   { 255, 255, 255, 0 }  // 5 White
};

inline static void scr_render_hgr_line_green(struct scr_t *scr, int line, uint16_t line_base, SDL_Surface *surface) {
   uint32_t green = scr->text_color;
   int x = 0;
   for (int i = 0; i < 40; i++) {
      uint8_t c = scr->two->screen_hgr_data[line_base + i];
      for (int j = 0; j < 7; j++) {
         uint32_t color = (c & (1 << j)) ? green : 0;

         int px = x * 3, py = line * 3;

         _set_pixel(surface, px+0, py+0, color);
         _set_pixel(surface, px+1, py+0, color);
         _set_pixel(surface, px+2, py+0, color);

         _set_pixel(surface, px+0, py+1, color);
         _set_pixel(surface, px+1, py+1, color);
         _set_pixel(surface, px+2, py+1, color);

         _set_pixel(surface, px+0, py+2, color);
         _set_pixel(surface, px+1, py+2, color);
         _set_pixel(surface, px+2, py+2, color);

         x++;
      }
   }
}

inline static void scr_render_hgr_line_color(struct scr_t *scr, int line, uint16_t line_base, SDL_Surface *surface) {
   uint32_t pixels[280], x = 0;
   for (int i = 0; i < 40; i++) {
      uint8_t c = scr->two->screen_hgr_data[line_base + i];
      for (int j = 0; j < 7; j++) {
         if (c & (1 << j)) {
            if (x % 2 == 0) {
               if (c & 0x80) {
                  pixels[x] = scr->hires_colors[1]; // Blue
               } else {
                  pixels[x] = scr->hires_colors[2]; // Purple
               }
            } else {
               if (c & 0x80) {
                  pixels[x] = scr->hires_colors[4]; // Red
               } else {
                  pixels[x] = scr->hires_colors[3]; // Green
               }
            }
         } else {
            pixels[x] = scr->hires_colors[0]; // Black
         }
         x++;
      }
   }

   // Flip adject pixels to white

   for (int i = 0; i < (280-1); i++) {
      if (pixels[i] && pixels[i+1]) {
         pixels[i] = scr->hires_colors[5]; // White
      }
   }

   // Now draw them

   for (x = 0; x < 280; x++) {
      int px = x * 3, py = line * 3;
      uint32_t color = pixels[x];
      _set_pixel(surface, px+0, py+0, color);
      _set_pixel(surface, px+1, py+0, color);
      _set_pixel(surface, px+2, py+0, color);
      _set_pixel(surface, px+0, py+1, color);
      _set_pixel(surface, px+1, py+1, color);
      _set_pixel(surface, px+2, py+1, color);
      _set_pixel(surface, px+0, py+2, color);
      _set_pixel(surface, px+1, py+2, color);
      _set_pixel(surface, px+2, py+2, color);
   }
}

inline static void scr_render_hgr_screen(struct scr_t *scr, bool flash) {
   // Render graphics
   int lines = (scr->two->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED) ? 160  : 192;
   uint16_t hgr_base = hgr_page_offsets[scr->two->screen_page];
   SDL_Surface *surface = SDL_GetWindowSurface(scr->window);
   for (int line = 0; line < lines; line++) {
      uint16_t line_base = hgr_base + hgr_line_offsets[line];
      if (scr->color_scheme == EWM_SCR_COLOR_SCHEME_COLOR) {
	scr_render_hgr_line_color(scr, line, line_base, surface);
      } else {
	scr_render_hgr_line_green(scr, line, line_base, surface);
      }
   }

   // Render bottom 4 lines of text
   if (scr->two->screen_graphics_style == EWM_A2P_SCREEN_GRAPHICS_STYLE_MIXED) {
      for (int row = 20; row < 24; row++) {
         for (int column = 0; column < 40; column++) {
            scr_render_character(scr, row, column, flash);
         }
      }
   }
}

// TODO This is the only actual API exposed

int ewm_scr_init(struct scr_t *scr, struct ewm_two_t *two, SDL_Window *window, SDL_Renderer *renderer) {
   memset(scr, 0x00, sizeof(struct scr_t));
   scr->two = two;
   scr->window = window;
   scr->renderer = renderer;
   scr->chr = ewm_chr_create("rom/3410036.bin", EWM_CHR_ROM_TYPE_2716, renderer);
   if (scr->chr == NULL) {
      fprintf(stderr, "[SCR] Failed to initialize character generator\n");
      return -1;
   }

   // Cache colors for speed, to avoid calls to SDL_MapRGB
   SDL_Surface *surface = SDL_GetWindowSurface(window);
   if (scr->color_scheme == EWM_SCR_COLOR_SCHEME_MONOCHROME) {
     scr->text_color = SDL_MapRGB(surface->format, 47, 249, 64);
   } else {
     scr->text_color = SDL_MapRGB(surface->format, 255, 255, 255);
   }
   for (int i = 0; i < 6; i++) {
     scr->hires_colors[i] = SDL_MapRGB(surface->format, hires_colors[i].r, hires_colors[i].g, hires_colors[i].b);
   }
   for (int i = 0; i < 16; i++) {
     scr->lores_colors_color[i] = SDL_MapRGB(surface->format, lores_colors_color[i].r,
					     lores_colors_color[i].g, lores_colors_color[i].b);
     scr->lores_colors_green[i] = SDL_MapRGB(surface->format, lores_colors_green[i].r,
					     lores_colors_green[i].g, lores_colors_green[i].b);
   }   
   return 0;
}

struct scr_t *ewm_scr_create(struct ewm_two_t *two, SDL_Window *window, SDL_Renderer *renderer) {
   struct scr_t *scr = malloc(sizeof(struct scr_t));
   if (ewm_scr_init(scr, two, window, renderer) != 0) {
      free(scr);
      scr = NULL;
   }
   return scr;
}

void ewm_scr_destroy(struct scr_t *scr) {
   // TODO
}

void ewm_scr_update(struct scr_t *scr, int phase, int fps) {
   SDL_FillRect(SDL_GetWindowSurface(scr->window), NULL, 0);

   switch (scr->two->screen_mode) {
      case EWM_A2P_SCREEN_MODE_TEXT:
         scr_render_txt_screen(scr, phase >= (fps / 2));
         break;
      case EWM_A2P_SCREEN_MODE_GRAPHICS:
         switch (scr->two->screen_graphics_mode) {
            case EWM_A2P_SCREEN_GRAPHICS_MODE_LGR:
               scr_render_lgr_screen(scr, phase >= (fps / 2));
               break;
            case EWM_A2P_SCREEN_GRAPHICS_MODE_HGR:
               scr_render_hgr_screen(scr, phase >= (fps / 2));
               break;
         }
         break;
   }
}

void ewm_scr_set_color_scheme(struct scr_t *scr, int color_scheme) {
   scr->color_scheme = color_scheme;
}
