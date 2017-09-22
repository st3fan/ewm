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

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cpu.h"
#include "mem.h"

// The following two are our memory primitives that properly set go
// through the handler functions for all registered memory. They will
// take more time but do the right thing.

uint8_t mem_get_byte(struct cpu_t *cpu, uint16_t addr) {
   if (addr < cpu->ram_size) {
      return cpu->ram[addr];
   }

   struct mem_t *mem = cpu->mem;
   while (mem != NULL) {
      if (mem->enabled && addr >= mem->start && addr <= mem->end) {
         if (mem->read_handler != NULL && mem->flags & MEM_FLAGS_READ) {
            return ((mem_read_handler_t) mem->read_handler)((struct cpu_t*) cpu, mem, addr);
         }
      }
      mem = mem->next;
   }
   return 0;
}

void mem_set_byte(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
   if (addr < cpu->ram_size) {
      cpu->ram[addr] = v;
      return;
   }

   struct mem_t *mem = cpu->mem;
   while (mem != NULL) {
      if (mem->enabled && addr >= mem->start && addr <= mem->end) {
         if (mem->write_handler && mem->flags & MEM_FLAGS_WRITE) {
            ((mem_write_handler_t) mem->write_handler)((struct cpu_t*) cpu, mem, addr, v);
         }
         return;
      }
      mem = mem->next;
   }
}

// Getters

uint8_t mem_get_byte_abs(struct cpu_t *cpu, uint16_t addr) {
   if (addr < cpu->ram_size) {
      return cpu->ram[addr];
   }
   return mem_get_byte(cpu, addr);
}

uint8_t mem_get_byte_absx(struct cpu_t *cpu, uint16_t addr) {
   if (addr < cpu->ram_size) {
      return cpu->ram[addr + cpu->state.x];
   }
   return mem_get_byte(cpu, addr + cpu->state.x);
}

uint8_t mem_get_byte_absy(struct cpu_t *cpu, uint16_t addr) {
   return cpu->ram[addr + cpu->state.y];
}

uint8_t mem_get_byte_zpg(struct cpu_t *cpu, uint8_t addr) {
   return cpu->ram[addr];
}

uint8_t mem_get_byte_zpgx(struct cpu_t *cpu, uint8_t addr) {
   return cpu->ram[((uint16_t) addr + cpu->state.x) & 0x00ff];
}

uint8_t mem_get_byte_zpgy(struct cpu_t *cpu, uint8_t addr) {
   return cpu->ram[((uint16_t) addr + cpu->state.y) & 0x00ff];
}

uint8_t mem_get_byte_indx(struct cpu_t *cpu, uint8_t addr) {
   uint16_t a = *((uint16_t*) &cpu->ram[(addr + cpu->state.x) & 0x0ff]);
   if (a < cpu->ram_size) {
      return cpu->ram[a];
   }
   return mem_get_byte(cpu, a);
}

uint8_t mem_get_byte_indy(struct cpu_t *cpu, uint8_t addr) {
   uint16_t a = *((uint16_t*) &cpu->ram[addr]) + cpu->state.y;
   if (a < cpu->ram_size) {
      return cpu->ram[a];
   }
   return mem_get_byte(cpu, a);
}

uint8_t mem_get_byte_ind(struct cpu_t *cpu, uint8_t addr) {
   uint16_t a = *((uint16_t*) &cpu->ram[addr]);
   if (a < cpu->ram_size) {
      return cpu->ram[a];
   }
   return mem_get_byte(cpu, a);
}

uint16_t mem_get_word(struct cpu_t *cpu, uint16_t addr) {
  if (addr < cpu->ram_size) {
     return *((uint16_t*) &cpu->ram[addr]);
  }
  return ((uint16_t) mem_get_byte(cpu, addr+1) << 8) | (uint16_t) mem_get_byte(cpu, addr);
}

// Setters

void mem_set_byte_zpg(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
   cpu->ram[addr] = v;
}

void mem_set_byte_zpgx(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
   cpu->ram[((uint16_t) addr + cpu->state.x) & 0x00ff] = v;
}

void mem_set_byte_zpgy(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
   cpu->ram[((uint16_t) addr + cpu->state.y) & 0x00ff] = v;
}

void mem_set_byte_abs(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
   if (addr < cpu->ram_size) {
      cpu->ram[addr] = v;
      return;
   }
   mem_set_byte(cpu, addr, v);
}

void mem_set_byte_absx(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
   if (addr < cpu->ram_size) {
      cpu->ram[addr + cpu->state.x] = v;
      return;
   }
   mem_set_byte(cpu, addr + cpu->state.x, v);
}

void mem_set_byte_absy(struct cpu_t *cpu, uint16_t addr, uint8_t v) {
   if (addr < cpu->ram_size) {
      cpu->ram[addr + cpu->state.y] = v;
      return;
   }
   mem_set_byte(cpu, addr + cpu->state.y, v);
}

void mem_set_byte_indx(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
   uint16_t a = *((uint16_t*) &cpu->ram[(addr + cpu->state.x) & 0x0ff]);
   if (a < cpu->ram_size) {
      cpu->ram[a] = v;
      return;
   }
   mem_set_byte(cpu, a, v);
}

void mem_set_byte_indy(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
   uint16_t a = *((uint16_t*) &cpu->ram[addr]) + cpu->state.y;
   if (a < cpu->ram_size) {
      cpu->ram[a] = v;
      return;
   }
   mem_set_byte(cpu, a, v);
}

void mem_set_byte_ind(struct cpu_t *cpu, uint8_t addr, uint8_t v) {
   uint16_t a = *((uint16_t*) &cpu->ram[addr]);
   if (a < cpu->ram_size) {
      cpu->ram[a] = v;
      return;
   }
   mem_set_byte(cpu, a, v);
}

void mem_set_word(struct cpu_t *cpu, uint16_t addr, uint16_t v) {
  mem_set_byte(cpu, addr+0, (uint8_t) v); // TODO Did I do this right?
  mem_set_byte(cpu, addr+1, (uint8_t) (v >> 8));
}

/* MOD */

void mem_mod_byte_zpg(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_zpg(cpu, addr, op(cpu, mem_get_byte_zpg(cpu, addr)));
}

void mem_mod_byte_zpgx(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_zpgx(cpu, addr, op(cpu, mem_get_byte_zpgx(cpu, addr)));
}

void mem_mod_byte_zpgy(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_zpgy(cpu, addr, op(cpu, mem_get_byte_zpgy(cpu, addr)));
}

void mem_mod_byte_abs(struct cpu_t *cpu, uint16_t addr, mem_mod_t op) {
  mem_set_byte_abs(cpu, addr, op(cpu, mem_get_byte_abs(cpu, addr)));
}

void mem_mod_byte_absx(struct cpu_t *cpu, uint16_t addr, mem_mod_t op) {
  mem_set_byte_absx(cpu, addr, op(cpu, mem_get_byte_absx(cpu, addr)));
}

void mem_mod_byte_absy(struct cpu_t *cpu, uint16_t addr, mem_mod_t op) {
  mem_set_byte_absy(cpu, addr, op(cpu, mem_get_byte_absy(cpu, addr)));
}

void mem_mod_byte_indx(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_indx(cpu, addr, op(cpu, mem_get_byte_indx(cpu, addr)));
}

void mem_mod_byte_indy(struct cpu_t *cpu, uint8_t addr, mem_mod_t op) {
  mem_set_byte_indy(cpu, addr, op(cpu, mem_get_byte_indy(cpu, addr)));
}

// For parsing --memory options

struct ewm_memory_option_t *parse_memory_option(char *s) {
   char *type = strtok(s, ":");
   if (type == NULL || (strcmp(type, "ram") != 0 && strcmp(type, "rom") != 0)) {
      return NULL;
   }

   char *address = strtok(NULL, ":");
   if (address == NULL) {
      return NULL;
   }

   char *path = strtok(NULL, ":");
   if (path == NULL) {
      return NULL;
   }

   struct ewm_memory_option_t *m = (struct ewm_memory_option_t*) malloc(sizeof(struct ewm_memory_option_t));
   m->type = strcmp(type, "ram") == 0 ? EWM_MEMORY_TYPE_RAM : EWM_MEMORY_TYPE_ROM;
   m->path = path;
   m->address = atoi(address);
   m->next = NULL;

   return m;
}

int cpu_add_memory_from_options(struct cpu_t *cpu, struct ewm_memory_option_t *m) {
   while (m != NULL) {
      fprintf(stderr, "[EWM] Adding %s $%.4X %s\n", m->type == EWM_MEMORY_TYPE_RAM ? "RAM" : "ROM", m->address, m->path);
      if (m->type == EWM_MEMORY_TYPE_RAM) {
         if (cpu_add_ram_file(cpu, m->address, m->path) == NULL) {
            fprintf(stderr, "[MEM] Failed to add RAM from %s\n", m->path);
            return -1;
         }
      } else {
         if (cpu_add_rom_file(cpu, m->address, m->path) == NULL) {
            fprintf(stderr, "[MEM] Failed to add ROM from %s\n", m->path);
            return -1;
         }
      }
      m = m->next;
   }
   return 0;
}
