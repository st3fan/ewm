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
#include <stdio.h>

#include <readline/readline.h>
#include <readline/history.h>

#include <SDL2/SDL.h>

#include "cpu.h"
#include "dbg.h"

static int ewm_dbg_thread(void *object) {
   struct ewm_dbg_t *dbg = (struct ewm_dbg_t*) object;
   printf("Welcome to the EWM debugger\n");
   while (1) {
      // Wait for the CPU to pause
      while (dbg->cpu->status == EWM_CPU_STATUS_RUNNING) {
         // TODO Replace this with a condition variable
      }

      printf("%.4x: A=%.2x X=%.2x Y=%.2x SP=%.2x\n", dbg->cpu->state.pc, dbg->cpu->state.a,
         dbg->cpu->state.x, dbg->cpu->state.y, dbg->cpu->state.sp);

      char *line = readline("(ewm) ");

      if (strcmp(line, "quit") == 0) {
         exit(0);
      }

      if (strcmp(line, "run") == 0) {
         ewm_dbg_continue(dbg);
         continue;
      }

      if (strcmp(line, "br $ffef") == 0) {
         dbg_breakpoint_set(dbg, 0xffef);
      }

      printf("Sorry I don't understand that\n");
   }
   return 0;
}

int ewm_dbg_start(struct ewm_dbg_t *dbg) {
   SDL_Thread *thread = SDL_CreateThread(ewm_dbg_thread, "ewm: debugger", dbg);
   if (thread == NULL) {
      return -1;
   }
   return 0;
}

void ewm_dbg_pause(struct ewm_dbg_t *dbg) {
   dbg->cpu->status = EWM_CPU_STATUS_PAUSED;
}

void ewm_dbg_continue(struct ewm_dbg_t *dbg) {
   dbg->cpu->status = EWM_CPU_STATUS_RUNNING;
}

void ewm_dbg_breakpoint_set(struct ewm_dbg_t *dbg, uint16_t addr) {
   printf("Setting breakpoint at %.4x\n", addr);
}

struct ewm_dbg_t *ewm_dbg_create(struct cpu_t *cpu) {
   struct ewm_dbg_t *dbg = (struct ewm_dbg_t*) malloc(sizeof(struct ewm_dbg_t));
   if (dbg != NULL) {
      dbg->cpu = cpu;
   }
   return dbg;
}
