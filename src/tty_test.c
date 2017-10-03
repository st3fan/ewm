
#include <stdio.h>
#include <unistd.h>

#include "one.h"
#include "tty.h"

void test(struct ewm_tty_t *tty) {
   // TODO Setup

   for (int i = 0; i < (EWM_ONE_TTY_ROWS * EWM_ONE_TTY_COLUMNS); i++) {
      tty->screen_buffer[i] = 32 + (rand() % 64);
   }

   Uint64 start = SDL_GetPerformanceCounter();
   for (int i = 0; i < 1000; i++) {
      SDL_SetRenderDrawColor(tty->renderer, 0, 0, 0, 255);
      SDL_RenderClear(tty->renderer);

      ewm_tty_refresh(tty, 1, EWM_ONE_FPS);

      SDL_Texture *texture = SDL_CreateTextureFromSurface(tty->renderer, tty->surface);
      if (texture != NULL) {
         SDL_RenderCopy(tty->renderer, texture, NULL, NULL);
         SDL_DestroyTexture(texture);
      }

      SDL_RenderPresent(tty->renderer);
   }
   Uint64 now = SDL_GetPerformanceCounter();
   double total = (double)((now - start)*1000) / SDL_GetPerformanceFrequency();
   double per_screen = total / 1000.0;

   printf("%-20s %.3f/refresh\n", "tty", per_screen);
}

int main() {
   if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS) < 0) {
      fprintf(stderr, "Failed to initialize SDL: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Window *window = SDL_CreateWindow("ewm - tty_test", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, 280*3, 192*3, SDL_WINDOW_SHOWN);
   if (window == NULL) {
      fprintf(stderr, "Failed create window: %s\n", SDL_GetError());
      return 1;
   }

   SDL_Renderer *renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);
   if (renderer == NULL) {
      fprintf(stderr, "Failed to create renderer: %s\n", SDL_GetError());
      return 1;
   }

   SDL_RenderSetLogicalSize(renderer, 280, 192);

   sleep(3);

   struct ewm_one_t *one = ewm_one_create(EWM_ONE_MODEL_APPLE1, renderer);
   test(one->tty);

   SDL_DestroyWindow(window);
   SDL_DestroyRenderer(renderer);
   SDL_Quit();

   return 0;
}
