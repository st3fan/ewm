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

#include "cpu.h"
#include "mem.h"
#include "mmu.h"

//
// This file implements the Apple IIe MMU. The MMU replaces most
// memory related logic on older models. It is an integrated chip that
// can do bank switching, manage memory access, etc. It is really the
// central hub.
//
// It manages all system RAM and ROM by taking ownership of the
// complete address space. It also provides hooks to manage the
// extension slots.
//
// The MMU implements an Apple IIe Extended 80-Column Card. There is
// no way to turn this off.
//
// We implement a modern Apple IIe memory map as follows:
//
//  
//  $C100 - $C2FF (49408 - 49919): Extensions to System Monitor
//  $C300 - $C3FF (49920 - 50175): 80-Column Display Routines
//  $C400 - $C7FF (50176 - 51199): Self-Test Routines
//  $C800 - $CFFF (51200 - 53247): More 80-Column Display Routines
//  $D000 - $F7FF (53248 - 63487): Applesoft Interpreter
//  $F800 - $FFFF (63488 - 65535): System Monitor
//  $D000 - $DFFF (53248 - 57343): Bank-Switched RAM (2 Banks RAM, 1 Bank ROM)
//  $E000 - $FFFF (57344 - 65535): Bank-Switched RAM (1 Bank RAM, 1 Bank ROM)
//

static uint8_t mmu_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr);
static void mmu_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b);

// Public API

void mmu_init(struct ewm_mmu_t *mmu, struct cpu_t *cpu) {
   cpu_add_iom(cpu, 0x0000, 0xffff, mmu, mmu_read, mmu_write);
}

void mmu_insert_card(struct ewm_mmu_t *mmu, uint8_t index, struct ewm_crd_t *card) {
}

void mmu_remove_card(struct ewm_mmu_t *mmu, uint8_t index) {
}

// Private API

static uint8_t mmu_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   return 0;
}

static void mmu_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
}

