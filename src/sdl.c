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

int ewm_sdl_pixel_format(SDL_Renderer *renderer) {
   SDL_RendererInfo info;
   if (SDL_GetRendererInfo(renderer, &info) != 0) {
      return -1;
   }

   for (Uint32 i = 0; i < info.num_texture_formats; i++) {
      int format = info.texture_formats[i];
      if (format == SDL_PIXELFORMAT_ARGB8888 || format == SDL_PIXELFORMAT_RGBA8888 || format == SDL_PIXELFORMAT_RGB888) {
         return format;
      }
   }

   return -1;
}

int ewm_sdl_check_renderer(SDL_Renderer *renderer) {
   SDL_RendererInfo info;
   if (SDL_GetRendererInfo(renderer, &info) != 0) {
      printf("ewm: sdl: cannot get renderer info: %s\n", SDL_GetError());
      return -1;
   }

   if ((info.flags & SDL_RENDERER_ACCELERATED) == 0) {
      printf("ewm: sdl: require accelerated renderer\n");
      return -1;
   }

   if (ewm_sdl_pixel_format(renderer) == -1) {
      printf("ewm: sdl: cannot find supported pixel format (ARGB888, RGBA8888, RGB888)\n");
      return -1;
   }

   return 0;
}

uint32_t ewm_sdl_green(SDL_Renderer *renderer) {
   switch (ewm_sdl_pixel_format(renderer)) {
      case SDL_PIXELFORMAT_RGBA8888:
         return 0x00ff00ff;
      case SDL_PIXELFORMAT_ARGB8888:
         return 0xff00ff00;
      case SDL_PIXELFORMAT_RGB888:
         return 0x00ff0000;
   }
   return 0xffffff;
}
