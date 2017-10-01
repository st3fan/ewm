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

#include <fcntl.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>

#include <SDL2/SDL.h>

#include "sdl.h"
#include "chr.h"

static int _load_rom_data(char *rom_path, uint8_t rom_data[2048]) {
   int fd = open(rom_path, O_RDONLY);
   if (fd == -1) {
      return -1;
   }

   struct stat file_info;
   if (fstat(fd, &file_info) == -1) {
      close(fd);
      return -1;
   }

   if (file_info.st_size  != 2048) {
      close(fd);
      return -1;
   }

   if (read(fd, rom_data, file_info.st_size) != file_info.st_size) {
      close(fd);
      return -1;
   }

   close(fd);

   return 0;
}

static void _set_pixel(SDL_Surface * surface, int x, int y, Uint32 color) {
   uint32_t *pixel = (uint32_t*) ((uint8_t*) surface->pixels + (y * surface->pitch) + (x * sizeof(uint32_t)));
   *pixel = color;
}

static SDL_Texture *_generate_texture(SDL_Renderer *renderer, uint8_t rom_data[2048], int c, bool inverse) {
   SDL_Texture *texture = NULL;

   uint8_t character_data[8];
   for (int i = 0; i < 8; i++) {
      character_data[i] = rom_data[(c * 8) + i + 1];
      if (inverse) {
         character_data[i] ^= 0xff;
      }
   }

   SDL_Surface *surface = SDL_CreateRGBSurface(0, 7, 8, 32, 0x000000ff, 0x0000ff00, 0x00ff0000, 0xff000000);
   if (surface != NULL) {
      for (int y = 0; y < 8; y++) {
         for (int x = 0; x < 7; x++) {
            if (character_data[y] & (1 << x)) {
               _set_pixel(surface, (6-x), y, 0xffffffff);
            }
         }
      }
      texture = SDL_CreateTextureFromSurface(renderer, surface);
      if (texture == NULL) {
         fprintf(stderr, "Cannot generate RGBSurface: %s\n", SDL_GetError());
      }
   } else {
      fprintf(stderr, "Cannot generate Texture: %s\n", SDL_GetError());
   }

   return texture;
}

static uint32_t *_generate_bitmap(struct ewm_chr_t *chr, uint8_t rom_data[2048], int c, bool inverse) {
   uint32_t *pixels = (uint32_t*) malloc(4 * ewm_chr_width(chr) * ewm_chr_height(chr));
   if (pixels != NULL) {
      memset(pixels, 0x00, 4 * ewm_chr_width(chr) * ewm_chr_height(chr));

      uint8_t character_data[8];
      for (int i = 0; i < 8; i++) {
         character_data[i] = rom_data[(c * 8) + i + 1];
         if (inverse) {
            character_data[i] ^= 0xff;
         }
      }

      uint32_t *p = pixels;
      for (int y = 0; y < 8; y++) {
         for (int x = 6; x >= 0; x--) {
            if (character_data[y] & (1 << x)) {
               *p++ = chr->green;
            } else {
               *p++ = 0x00000000;
            }
         }
      }
   }

   return pixels;
}

static int ewm_chr_init(struct ewm_chr_t *chr, char *rom_path, int rom_type, SDL_Renderer *renderer) {
   if (rom_type != EWM_CHR_ROM_TYPE_2716) {
      return -1;
   }
   memset(chr, 0x00, sizeof(struct ewm_chr_t));

   chr->renderer = renderer;
   chr->green = ewm_sdl_green(renderer);

   uint8_t rom_data[2048];
   if (_load_rom_data(rom_path, rom_data) != 0) {
      return -1;
   }

   // Bitmaps

   // Normal Text
   for (int c = 0; c < 32; c++) {
      chr->bitmaps[0xc0 + c] = _generate_bitmap(chr, rom_data, c, false);
   }
   for (int c = 32; c < 64; c++) {
      chr->bitmaps[0xa0 + (c-32)] = _generate_bitmap(chr, rom_data, c, false);
   }

   // Inverse Text
   for (int c = 0; c < 32; c++) {
      chr->bitmaps[0x00 + c] = _generate_bitmap(chr, rom_data, c, true);
   }
   for (int c = 32; c < 64; c++) {
      chr->bitmaps[0x20 + (c-32)] = _generate_bitmap(chr, rom_data, c, true);
   }

   // TODO Flashing - Currently simply rendered as inverse
   for (int c = 0; c < 32; c++) {
      chr->bitmaps[0x40 + c] = _generate_bitmap(chr, rom_data, c, true);
   }
   for (int c = 32; c < 64; c++) {
      chr->bitmaps[0x60 + (c-32)] = _generate_bitmap(chr, rom_data, c, true);
   }

   // Textures

   // Normal Text
   for (int c = 0; c < 32; c++) {
      chr->textures[0xc0 + c] = _generate_texture(renderer, rom_data, c, false);
   }
   for (int c = 32; c < 64; c++) {
      chr->textures[0xa0 + (c-32)] = _generate_texture(renderer, rom_data, c, false);
   }

   // Inverse Text
   for (int c = 0; c < 32; c++) {
      chr->textures[0x00 + c] = _generate_texture(renderer, rom_data, c, true);
   }
   for (int c = 32; c < 64; c++) {
      chr->textures[0x20 + (c-32)] = _generate_texture(renderer, rom_data, c, true);
   }

   // TODO Flashing - Currently simply rendered as inverse
   for (int c = 0; c < 32; c++) {
      chr->textures[0x40 + c] = _generate_texture(renderer, rom_data, c, true);
   }
   for (int c = 32; c < 64; c++) {
      chr->textures[0x60 + (c-32)] = _generate_texture(renderer, rom_data, c, true);
   }

   return 0;
}

struct ewm_chr_t* ewm_chr_create(char *rom_path, int rom_type, SDL_Renderer *renderer) {
   struct ewm_chr_t *chr = (struct ewm_chr_t*) malloc(sizeof(struct ewm_chr_t));
   int ret = ewm_chr_init(chr, rom_path, rom_type, renderer);
   if (ret != 0) {
      free(chr);
      chr = NULL;
   }
   return chr;
}

int ewm_chr_width(struct ewm_chr_t* chr) {
   return 7; // TODO Should be based on the ROM type?
}

int ewm_chr_height(struct ewm_chr_t* chr) {
   return 8; // TODO Should be based on the ROM type?
}
