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
#include <string.h>

#include "cpu.h"
#include "pia.h"

// This implements a 6820 Peripheral I/O Adapter. On the Apple I this
// is what connects the keyboard and display logic to the CPU. The
// implementation is not complete but does enough to support how the
// keyboard and display are hooked up.

static uint8_t pia_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   struct ewm_pia_t *pia = (struct ewm_pia_t*) mem->obj;
   switch (addr) {
      case EWM_A1_PIA6820_KBD_DDR:
         if (pia->ctla & 0b00000100) {
            pia->ctla &= 0b01111111; // Clear IRQA1
            return (pia->outa & pia->ddra) | (pia->ina & ~pia->ddra);
         } else {
            return pia->ddra;
         }
         break;
      case EWM_A1_PIA6820_KBD_CTL:
         return pia->ctla;
         break;
      case EWM_A1_PIA6820_DSP_DDR:
         if (pia->ctlb & 0b00000100) {
            return (pia->outb & pia->ddrb) | (pia->inb & ~pia->ddrb);
         } else {
            return pia->ddrb;
         }
         break;
      case EWM_A1_PIA6820_DSP_CTL:
         return pia->ctlb;
         break;
   }
   return 0;
}

static void pia_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t v) {
   struct ewm_pia_t *pia = (struct ewm_pia_t*) mem->obj;
   switch (addr) {
      case EWM_A1_PIA6820_KBD_DDR:
         // Check B2 (DDR Access)
         if (pia->ctla & 0b00000100) {
            // Write output register and run callback
            pia->outa = v;
            if (pia->callback) {
               pia->callback(pia, pia->callback_obj, EWM_PIA6820_DDRA, v);
            }
         } else {
            // Write DDR register
            pia->ddra = v;
            // TODO Do we need a callback? Not relevant for Apple 1?
         }
         break;
      case EWM_A1_PIA6820_KBD_CTL:
         pia->ctla = (v & 0b00111111);
         break;
      case EWM_A1_PIA6820_DSP_DDR:
         // Check B2 (DDR Access)
         if (pia->ctlb & 0b00000100) {
            // Write output register and run callback
            pia->outb = v;
            if (pia->callback) {
               pia->callback(pia, pia->callback_obj, EWM_PIA6820_DDRB, v);
            }
         } else {
            // Write DDR register
            pia->ddrb = v;
            // TODO Do we need a callback? Not relevant for Apple 1?
         }
         break;
      case EWM_A1_PIA6820_DSP_CTL:
         pia->ctlb = (v & 0b00111111);
         break;
   }
}

static int ewm_pia_init(struct ewm_pia_t *pia, struct cpu_t *cpu) {
   memset(pia, 0, sizeof(struct ewm_pia_t));
   cpu_add_iom(cpu, EWM_A1_PIA6820_ADDR, EWM_A1_PIA6820_ADDR + EWM_A1_PIA6820_LENGTH - 1, pia, pia_read, pia_write);
   return 0;
}

struct ewm_pia_t *ewm_pia_create(struct cpu_t *cpu) {
   struct ewm_pia_t *pia = (struct ewm_pia_t*) malloc(sizeof(struct ewm_pia_t));
   if (ewm_pia_init(pia, cpu) != 0) {
      free(pia);
      pia = NULL;
   }
   return pia;
}

void ewm_pia_destroy(struct ewm_pia_t *pia) {
   free(pia);
}

void ewm_pia_set_outa(struct ewm_pia_t *pia, uint8_t v) {
   pia->outa = v;
}

void ewm_pia_set_ina(struct ewm_pia_t *pia, uint8_t v) {
   pia->ina = v;
}

void ewm_pia_set_outb(struct ewm_pia_t *pia, uint8_t v) {
   pia->outb = v;
}

void ewm_pia_set_inb(struct ewm_pia_t *pia, uint8_t v) {
   pia->inb = v;
}

void ewm_pia_set_irqa1(struct ewm_pia_t *pia) {
   pia->ctla |= 0b10000000; // Set IRQA1
}
