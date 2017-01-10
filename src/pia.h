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

#ifndef PIA_H
#define PIA_H

#include <stdint.h>
#include "cpu.h"

#define	EWM_PIA6820_DDRA	0
#define	EWM_PIA6820_CTLA	1
#define	EWM_PIA6820_DDRB	2
#define	EWM_PIA6820_CTLB	3

#define EWM_A1_PIA6820_ADDR   0xd010
#define EWM_A1_PIA6820_LENGTH 0x0100

#define EWM_A1_PIA6820_KBD_DDR (EWM_A1_PIA6820_ADDR + EWM_PIA6820_DDRA)
#define EWM_A1_PIA6820_KBD_CTL (EWM_A1_PIA6820_ADDR + EWM_PIA6820_CTLA)
#define EWM_A1_PIA6820_DSP_DDR (EWM_A1_PIA6820_ADDR + EWM_PIA6820_DDRB)
#define EWM_A1_PIA6820_DSP_CTL (EWM_A1_PIA6820_ADDR + EWM_PIA6820_CTLB)

struct ewm_pia_t;

typedef void (*ewm_pia_callback_t)(struct ewm_pia_t *pia, void *obj, uint8_t ddr, uint8_t v);

struct ewm_pia_t {
   uint8_t ina;
   uint8_t outa;
   uint8_t ddra;
   uint8_t ctla;
   uint8_t inb;
   uint8_t outb;
   uint8_t ddrb;
   uint8_t ctlb;
   ewm_pia_callback_t callback;
   void *callback_obj;
};

struct ewm_pia_t *ewm_pia_create(struct cpu_t *cpu);
void ewm_pia_destroy(struct ewm_pia_t *pia);

void ewm_pia_set_outa(struct ewm_pia_t *pia, uint8_t v);
void ewm_pia_set_ina(struct ewm_pia_t *pia, uint8_t v);

void ewm_pia_set_outb(struct ewm_pia_t *pia, uint8_t v);
void ewm_pia_set_inb(struct ewm_pia_t *pia, uint8_t v);

void ewm_pia_set_irqa1(struct ewm_pia_t *pia);

#endif
