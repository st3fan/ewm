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

// Apple 1 / 8K RAM / WOZ Monitor

static int setup_apple1(struct cpu_t *cpu) {
   struct pia_t *pia = malloc(sizeof(struct pia_t));
   pia_init(pia);
   pia_trace(pia, 0);
   cpu_add_ram(cpu, 0x0000, 8 * 1024);
   cpu_add_rom_file(cpu, 0xff00, "roms/apple1.com");
   cpu_add_iom(cpu, EWM_A1_PIA6820_ADDR, EWM_A1_PIA6820_LENGTH, pia, pia_read, pia_write);
   return 0;
}

// Replica 1 / 32K RAM / Krusader Assembler & Monitor

static int setup_replica1(struct cpu_t *cpu) {
   struct pia_t *pia = malloc(sizeof(struct pia_t));
   pia_init(pia);
   pia_trace(pia, 0);
   cpu_add_ram(cpu, 0x0000, 32 * 1024);
   cpu_add_rom_file(cpu, 0xe000, "roms/krusader.rom");
   cpu_add_iom(cpu, EWM_A1_PIA6820_ADDR, EWM_A1_PIA6820_LENGTH, pia, pia_read, pia_write);
   return 0;
}

// Apple ][+ / 48K RAM / Original ROMs

static int setup_apple2plus(struct cpu_t *cpu) {
   cpu_add_ram(cpu, 0x0000, 48 * 1024);
   cpu_add_rom_file(cpu, 0xd000, "roms/a2p.rom");
   return 0;
}

// Machine Setup

typedef int (*ewm_machine_setup_f)(struct cpu_t *cpu);

struct ewm_machine_t {
   char *name;
   char *description;
   ewm_machine_setup_f setup;
};

static struct ewm_machine_t machines[] = {
   { "apple1",     "Apple 1",   setup_apple1 },
   { "replica1",   "Replica 1", setup_replica1 },
   { "apple2plus", "Apple ][+", setup_apple2plus },
   { NULL,         NULL,        NULL }
};

static struct option options[] = {
   { "machine", required_argument, NULL, 'm' },
   { "strict",  no_argument,       NULL, 's' },
   { "trace",   optional_argument, NULL, 't' },
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
      }
   }

   argc -= optind;
   argv += optind;

   if (machine == NULL) {
      fprintf(stderr, "Usage: ewm --machine apple1|replica1|apple2plus\n");
      exit(1);
   }

   struct cpu_t cpu;
   cpu_init(&cpu);
   cpu_strict(&cpu, strict);
   cpu_trace(&cpu, trace_path);

   machine->setup(&cpu);

   switch (cpu_boot(&cpu)) {
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

   cpu_shutdown(&cpu);

   return 0;
}
