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
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <getopt.h>
#include <string.h>

#include "cpu.h"
#include "mem.h"
#include "pia.h"

#include "a2p.h"
#include "scr.h"

// Apple 1 / 6502 / 8K RAM / WOZ Monitor

static int setup_apple1(struct cpu_t *cpu) {
   cpu_init(cpu, EWM_CPU_MODEL_6502);
   struct pia_t *pia = malloc(sizeof(struct pia_t));
   pia_init(pia);
   cpu_add_ram(cpu, 0x0000, 8 * 1024 - 1);
   cpu_add_rom_file(cpu, 0xff00, "roms/apple1.rom");
   cpu_add_iom(cpu, EWM_A1_PIA6820_ADDR, EWM_A1_PIA6820_ADDR + EWM_A1_PIA6820_LENGTH - 1, pia, pia_read, pia_write);
   return 0;
}

// Replica 1 / 65C02 / 32K RAM / Krusader Assembler & Monitor

static int setup_replica1(struct cpu_t *cpu) {
   cpu_init(cpu, EWM_CPU_MODEL_65C02);
   struct pia_t *pia = malloc(sizeof(struct pia_t));
   pia_init(pia);
   cpu_add_ram(cpu, 0x0000, 32 * 1024 - 1);
   cpu_add_rom_file(cpu, 0xe000, "roms/krusader.rom");
   cpu_add_iom(cpu, EWM_A1_PIA6820_ADDR, EWM_A1_PIA6820_ADDR + EWM_A1_PIA6820_LENGTH - 1, pia, pia_read, pia_write);
   return 0;
}

// Apple ][+ / 6502 / 48K RAM / Original ROMs

static struct a2p_t *a2p;

static int setup_apple2plus(struct cpu_t *cpu) {
   cpu_init(cpu, EWM_CPU_MODEL_6502);
   a2p = malloc(sizeof(struct a2p_t));
   a2p_init(a2p, cpu);
   return 0;
}

// Machine Setup

typedef int (*ewm_machine_setup_f)(struct cpu_t *cpu);

struct ewm_machine_t {
   char *name;
   char *description;
   int graphics;
   ewm_machine_setup_f setup;
};

#define EWM_MEMORY_TYPE_RAM 0
#define EWM_MEMORY_TYPE_ROM 1

struct ewm_memory_t {
   int type;
   char *path;
   uint16_t address;
   struct ewm_memory_t *next;
};

struct ewm_memory_t *parse_memory(char *s) {
   char *type = strsep(&s, ":");
   if (type == NULL) { // || (strcmp(type, "ram") && strcmp(type, "rom"))) {
      printf("type fail\n");
      return NULL;
   }

   char *address = strsep(&s, ":");
   if (address == NULL) {
      printf("address fail\n");
      return NULL;
   }

   char *path = strsep(&s, ":");
   if (path == NULL) {
      printf("path fail\n");
      return NULL;
   }

   struct ewm_memory_t *m = (struct ewm_memory_t*) malloc(sizeof(struct ewm_memory_t));
   m->type = strcmp(type, "ram") ? EWM_MEMORY_TYPE_RAM : EWM_MEMORY_TYPE_ROM;
   m->path = path;
   m->address = atoi(address);
   m->next = NULL;

   return m;
}

static struct ewm_machine_t machines[] = {
   { "apple1",     "Apple 1",   false, setup_apple1 },
   { "replica1",   "Replica 1", false, setup_replica1 },
   { "apple2plus", "Apple ][+", true,  setup_apple2plus },
   { NULL,         NULL,        false, NULL }
};

static struct option options[] = {
   { "machine", required_argument, NULL, 'm' },
   { "strict",  no_argument,       NULL, 's' },
   { "trace",   optional_argument, NULL, 't' },
   { "memory",  required_argument, NULL, 'x' },
   { NULL,      0,                 NULL, 0   }
};

static struct ewm_machine_t *machine_with_name(char *name) {
   for (struct ewm_machine_t *m = machines; m->name != NULL; m++) {
      if (strcmp(m->name, name) == 0) {
         return m;
      }
   }
   return NULL;
}

int main(int argc, char **argv) {
   struct ewm_machine_t *machine = &machines[0];
   bool strict = false;
   char *trace_path = NULL;
   struct ewm_memory_t *memory;

   char ch;
   while ((ch = getopt_long(argc, argv, "m:", options, NULL)) != -1) {
      switch (ch) {
         case 'm':
            machine = machine_with_name(optarg);
            break;
         case 's':
            strict = true;
            break;
         case 't':
            trace_path = optarg ? optarg : "/dev/stderr";
            break;
         case 'x': {
            struct ewm_memory_t *m = parse_memory(optarg);
            if (m == NULL) {
               exit(1);
            }
            if (memory == NULL) {
               memory = m;
            } else {
               memory->next = m;
            }
            break;
         }
      }
   }

   argc -= optind;
   argv += optind;

   if (machine == NULL) {
      fprintf(stderr, "Usage: ewm --machine apple1|replica1|apple2plus\n");
      exit(1);
   }

   cpu_setup();

   fprintf(stderr, "[EWM] Starting up %s\n", machine->description);

   struct cpu_t cpu;

   machine->setup(&cpu);

   struct ewm_memory_t *m = memory;
   while (m != NULL) {
      fprintf(stderr, "[EWM] Adding %s $%.4X %s\n", m->type == EWM_MEMORY_TYPE_RAM ? "RAM" : "ROM", m->address, m->path);
      if (m->type == EWM_MEMORY_TYPE_RAM) {
         if (cpu_add_ram_file(&cpu, m->address, m->path) == NULL) {
            fprintf(stderr, "[EWM] Failed to add RAM from %s\n", m->path);
            exit(1);
         }
      } else {
         if (cpu_add_rom_file(&cpu, m->address, m->path) == NULL) {
            fprintf(stderr, "[EWM] Failed to add ROM from %s\n", m->path);
            exit(1);
         }
      }
      m = m->next;
   }

   cpu_strict(&cpu, strict);
   cpu_trace(&cpu, trace_path);

   cpu_reset(&cpu);

   if (machine->graphics) {
      scr_init();
      scr_main(&cpu, a2p);
   } else {
      while (true) {
         switch (cpu_step(&cpu)) {
            case EWM_CPU_ERR_UNIMPLEMENTED_INSTRUCTION:
               fprintf(stderr, "CPU: Exited because of unimplemented instructions 0x%.2x at 0x%.4x\n",
                       mem_get_byte(&cpu, cpu.state.pc), cpu.state.pc);
               break;
            case EWM_CPU_ERR_STACK_OVERFLOW:
               fprintf(stderr, "CPU: Exited because of stack overflow at 0x%.4x\n", cpu.state.pc);
               break;
            case EWM_CPU_ERR_STACK_UNDERFLOW:
               fprintf(stderr, "CPU: Exited because of stack underflow at 0x%.4x\n", cpu.state.pc);
               break;
         }
      }
   }

   cpu_shutdown(&cpu);

   return 0;
}
