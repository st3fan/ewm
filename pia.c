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

static void pia_dsp_write(uint8_t b) {
   //fprintf(stderr, "PIA: Sending to display: %.2x / %.2x\n", b, b & 0x7f);
   b &= 0b01111111;
   if (b == '\r') {
      b = '\n';
   }
   addch(b);
   refresh();
}

/* static int pia_kbd_read() { */
/*    int c = getch(); */
/* } */

/* static uint8_t pia_kbd_read() { */
/*    return getchar(); */
/* } */

void pia_init(struct pia_t *pia) {
   initscr();
   raw();
   noecho();
   pia->a = 0;
   pia->cra = 0;
   pia->b = 0;
   pia->crb = 0;
}

void pia_trace(struct pia_t *pia, uint8_t trace) {
   pia->trace = trace;
}

uint8_t pia_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct pia_t *pia = (struct pia_t*) mem->obj;
   uint8_t result = 0;
   switch (addr) {
      case 0xd010: /* KBD */
         result = pia->a;
         break;
      case 0xd011: /* KBDCR */ {
         int c = getch();
         if (c != ERR) {
            /* TODO: Remove this, this is not how we want to stop the emulator */
            if (c == 3) {
               exit(1);
            }
            if (c == '\n') {
               c = '\r';
            }
            pia->a = c | 0x80;
         }
         result = (c == ERR) ? 0x00 : 0x80;
         break;
      }
      case 0xd012: /* DSP */
         result = 0;
         break;
      case 0xd013: /* DSPCR */
         result = 0;
         break;
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
      case 0xd010: /* KBD */
         break;
      case 0xd011: /* KBDCR */
         pia->cra = b;
         break;
      case 0xd012: /* DSP */
         if (pia->crb != 0x00) { /* TODO: Check the actual flag */
            pia_dsp_write(b);
         }
         break;
      case 0xd013: /* DSPCR */
         pia->crb = b;
         break;
   }
}
