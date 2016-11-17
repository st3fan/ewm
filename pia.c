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
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <curses.h>

#include "cpu.h"
#include "pia.h"

// This implements a 6820 Peripheral I/O Adapter. On the Apple I this
// is what connects the keyboard and display logic to the CPU.

static void pia_dsp_write(uint8_t b) {
   b &= 0b01111111;
   if (b == '\r') {
      b = '\n';
   }
   addch(b);
   refresh();
}

void pia_init(struct pia_t *pia) {
   initscr();
   raw();
   noecho();
   pia->a = 0;
   pia->cra = 0;
   pia->b = 0;
   pia->crb = 0;
}

void pia_trace(struct pia_t *pia, uint8_t enable) {
   pia->trace = enable;
}

uint8_t pia_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct pia_t *pia = (struct pia_t*) mem->obj;

   uint8_t result = 0;

   switch (addr) {
      case EWM_A1_PIA6820_KBD: {
         result = pia->a;
         break;
      }

      case EWM_A1_PIA6820_KBDCR: {
         int c = getch();
         if (c != ERR) {
            /* TODO: Remove this, this is not how we want to stop the emulator */
            if (c == 3) {
               exit(1);
            }
            if (c == '\n') {
               c = '\r';
            }
            pia->a = c | 0x80; // Set the high bit - WHat is up with the high bits. Document this.
         }
         result = (c == ERR) ? 0x00 : 0x80;
         break;
      }

      case EWM_A1_PIA6820_DSP: {
         result = 0;
         break;
      }

      case EWM_A1_PIA6820_DSPCR: {
         result = 0;
         break;
      }
   }

   if (pia->trace) {
      fprintf(stderr, "PIA: READ BYTE %.2X FROM %.4X\n", result, addr);
   }

   return result;
}

void pia_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   struct pia_t *pia = (struct pia_t*) mem->obj;

   if (pia->trace) {
      fprintf(stderr, "PIA: WRITING BYTE %.2X TO %.4X\n", b, addr);
   }

   switch (addr) {
      case EWM_A1_PIA6820_KBD: { /* KBD */
         break;
      }

      case EWM_A1_PIA6820_KBDCR: { /* KBDCR */
         pia->cra = b;
         break;
      }

      case EWM_A1_PIA6820_DSP: { /* DSP */
         if (pia->crb != 0x00) { /* TODO: Check the actual flag */
            pia_dsp_write(b);
         }
         break;
      }

      case EWM_A1_PIA6820_DSPCR: { /* DSPCR */
         pia->crb = b;
         break;
      }
   }
}
