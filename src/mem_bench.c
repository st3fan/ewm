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
#include <stdint.h>
#include <time.h>

#include "cpu.h"
#include "mem.h"
#include "utl.h"

#if 0
#define MEM_BENCH_ITERATIONS (100 * 1000 * 1000)

#define MEM_GET_TEST(NAME, ADDR) \
   void test_ ## NAME ## _ ## ADDR(struct cpu_t *cpu) { \
       for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) { \
          (void) NAME(cpu, ADDR); \
       } \
   }

#define MEM_SET_TEST(NAME, ADDR) \
   void test_ ## NAME ## _ ## ADDR(struct cpu_t *cpu) { \
       for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) { \
          (void) NAME(cpu, ADDR, 0xaa); \
       } \
   }

#define RUN_TEST(NAME, ADDR) test(cpu, #NAME, test_ ## NAME ## _ ## ADDR)

typedef void (*test_run_t)(struct cpu_t *cpu);

void test_cpu_push_byte(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      _cpu_push_byte(cpu, 0xaa);
   }
}

void test_cpu_pull_byte(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      (void) _cpu_pull_byte(cpu);
   }
}

void test_cpu_push_word(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      _cpu_push_word(cpu, 0xaeae);
   }
}

void test_cpu_pull_word(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      (void) _cpu_pull_word(cpu);
   }
}

void test_mem_get_byte(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      (void) mem_get_byte(cpu, 0x1234);
   }
}

void test_mem_set_byte(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      mem_set_byte(cpu, 0x1234, 0xaa);
   }
}

void test_mem_get_byte_zp(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      (void) mem_get_byte(cpu, 0x0011);
   }
}

void test_mem_set_byte_zp(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      mem_set_byte(cpu, 0x0011, 0xaa);
   }
}

void test_mem_get_byte_stack(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      (void) mem_get_byte(cpu, 0x0111);
   }
}

void test_mem_set_byte_stack(struct cpu_t *cpu) {
   for (uint64_t i = 0; i < MEM_BENCH_ITERATIONS; i++) {
      mem_set_byte(cpu, 0x0111, 0xaa);
   }
}

MEM_GET_TEST(mem_get_byte, 0x1234)
MEM_GET_TEST(mem_get_byte_abs, 0x1234)
MEM_GET_TEST(mem_get_byte_absx, 0x1234)
MEM_GET_TEST(mem_get_byte_absy, 0x1234)
MEM_GET_TEST(mem_get_byte_zpg, 0x12)
MEM_GET_TEST(mem_get_byte_zpgx, 0x12)
MEM_GET_TEST(mem_get_byte_zpgy, 0x12)
MEM_GET_TEST(mem_get_byte_ind, 0x12)
MEM_GET_TEST(mem_get_byte_indx, 0x12)
MEM_GET_TEST(mem_get_byte_indy, 0x12)

MEM_SET_TEST(mem_set_byte, 0x1234)
MEM_SET_TEST(mem_set_byte_abs, 0x1234)
MEM_SET_TEST(mem_set_byte_absx, 0x1234)
MEM_SET_TEST(mem_set_byte_absy, 0x1234)
MEM_SET_TEST(mem_set_byte_zpg, 0x12)
MEM_SET_TEST(mem_set_byte_zpgx, 0x12)
MEM_SET_TEST(mem_set_byte_zpgy, 0x12)
MEM_SET_TEST(mem_set_byte_ind, 0x12)
MEM_SET_TEST(mem_set_byte_indx, 0x12)
MEM_SET_TEST(mem_set_byte_indy, 0x12)

void test(struct cpu_t *cpu, char *name, test_run_t test_run) {
   struct timespec start;
   if (clock_gettime(CLOCK_REALTIME, &start) != 0) {
      perror("Cannot get time");
      exit(1);
   }

   test_run(cpu);

   struct timespec now;
   if (clock_gettime(CLOCK_REALTIME, &now) != 0) {
      perror("Cannot get time");
      exit(1);
   }

   uint64_t duration_ms = (now.tv_sec * 1000 + (now.tv_nsec / 1000000))
      - (start.tv_sec * 1000 + (start.tv_nsec / 1000000));

   printf("%-32s %8llu\n", name, duration_ms);
}
#endif

int main(int argc, char **argv) {
#if 0
   struct cpu_t *cpu = cpu_create(EWM_CPU_MODEL_6502);
   cpu_add_ram_data(cpu, 0, 0xffff, malloc(0xffff));
   cpu_reset(cpu);

   printf("-------------------------------- --------\n");
   test(cpu, "_cpu_push_byte", test_cpu_push_byte);
   test(cpu, "_cpu_pull_byte", test_cpu_pull_byte);
   test(cpu, "_cpu_push_word", test_cpu_push_word);
   test(cpu, "_cpu_pull_word", test_cpu_pull_word);

   printf("-------------------------------- --------\n");
   RUN_TEST(mem_get_byte, 0x1234);
   RUN_TEST(mem_get_byte_abs, 0x1234);
   RUN_TEST(mem_get_byte_absx, 0x1234);
   RUN_TEST(mem_get_byte_absy, 0x1234);
   RUN_TEST(mem_get_byte_zpg, 0x12);
   RUN_TEST(mem_get_byte_zpgx, 0x12);
   RUN_TEST(mem_get_byte_zpgy, 0x12);
   RUN_TEST(mem_get_byte_ind, 0x12);
   RUN_TEST(mem_get_byte_indx, 0x12);
   RUN_TEST(mem_get_byte_indy, 0x12);

   printf("-------------------------------- --------\n");
   RUN_TEST(mem_set_byte, 0x1234);
   RUN_TEST(mem_set_byte_abs, 0x1234);
   RUN_TEST(mem_set_byte_absx, 0x1234);
   RUN_TEST(mem_set_byte_absy, 0x1234);
   RUN_TEST(mem_set_byte_zpg, 0x12);
   RUN_TEST(mem_set_byte_zpgx, 0x12);
   RUN_TEST(mem_set_byte_zpgy, 0x12);
   RUN_TEST(mem_set_byte_ind, 0x12);
   RUN_TEST(mem_set_byte_indx, 0x12);
   RUN_TEST(mem_set_byte_indy, 0x12);
#endif
}
