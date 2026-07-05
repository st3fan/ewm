// Golden-trace capture harness for the C-to-Rust rewrite (see REWRITE.md,
// Phase 2). Runs the first N instructions of the Klaus Dormann 6502
// functional test on the C emulator and prints one normalized line of
// pre-step CPU state per instruction:
//
//     PC A X Y SP P     (uppercase hex, fixed columns)
//
// Built and run by scripts/gen-golden-trace.sh — it compiles this file
// against the unmodified sources in src/, so nothing in the C tree changes.

#include <stdio.h>
#include <stdlib.h>

#include "cpu.h"
#include "mem.h"

int main(int argc, char **argv) {
   long steps = 100000;
   if (argc > 1) {
      steps = strtol(argv[1], NULL, 10);
   }

   struct cpu_t *cpu = cpu_create(EWM_CPU_MODEL_6502);
   if (cpu_add_ram_file(cpu, 0x0000, "rom/6502_functional_test.bin") == NULL) {
      fprintf(stderr, "cannot load rom/6502_functional_test.bin (run from src/)\n");
      return 1;
   }
   cpu_reset(cpu);
   cpu->state.pc = 0x0400;

   for (long i = 0; i < steps; i++) {
      printf("%04X %02X %02X %02X %02X %02X\n", cpu->state.pc, cpu->state.a,
             cpu->state.x, cpu->state.y, cpu->state.sp, _cpu_get_status(cpu));
      cpu_step(cpu);
   }

   return 0;
}
