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
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <inttypes.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/stat.h>

#include "cpu.h"
#include "ins.h"
#include "mem.h"
#include "fmt.h"
#include "lua.h"

// Stack management.

void _cpu_push_byte(struct cpu_t *cpu, uint8_t b) {
   cpu->ram[0x0100 + cpu->state.sp--] = b;
}

void _cpu_push_word(struct cpu_t *cpu, uint16_t w) {
   cpu->ram[0x0100 + cpu->state.sp--] = w >> 8;
   cpu->ram[0x0100 + cpu->state.sp--] = w;
}

uint8_t _cpu_pull_byte(struct cpu_t *cpu) {
   return cpu->ram[0x0100 + ++cpu->state.sp];
}

uint16_t _cpu_pull_word(struct cpu_t *cpu) {
   uint16_t w = (uint16_t) cpu->ram[0x0100 + ++cpu->state.sp];
   return w | ((uint16_t) cpu->ram[0x0100 + ++cpu->state.sp] << 8);
}

uint8_t _cpu_stack_free(struct cpu_t *cpu) {
   return cpu->state.sp;
}

uint8_t _cpu_stack_used(struct cpu_t *cpu) {
   return 0xff - cpu->state.sp;
}

// Because we keep the processor status bits in separate fields, we
// need a function to combine them into a single register. This is
// only used when we need to push the register on the stack for
// interupt handlers. If this turns out to be inefficient then they
// can be stored in their native form in a byte.

uint8_t _cpu_get_status(struct cpu_t *cpu) {
  return 0x30
    | (((cpu->state.n != 0) & 0x01) << 7)
    | (((cpu->state.v != 0) & 0x01) << 6)
    | (((cpu->state.b != 0) & 0x01) << 4)
    | (((cpu->state.d != 0) & 0x01) << 3)
    | (((cpu->state.i != 0) & 0x01) << 2)
    | (((cpu->state.z != 0) & 0x01) << 1)
    | (((cpu->state.c != 0) & 0x01) << 0);
}

void _cpu_set_status(struct cpu_t *cpu, uint8_t status) {
  cpu->state.n = (status & (1 << 7));
  cpu->state.v = (status & (1 << 6));
  cpu->state.b = (status & (1 << 4));
  cpu->state.d = (status & (1 << 3));
  cpu->state.i = (status & (1 << 2));
  cpu->state.z = (status & (1 << 1));
  cpu->state.c = (status & (1 << 0));
}

static int cpu_execute_instruction(struct cpu_t *cpu) {
   // Fetch instruction
   struct cpu_instruction_t *i = &cpu->instructions[mem_get_byte(cpu, cpu->state.pc)];

   // Remember and advance the pc
   uint16_t pc = cpu->state.pc;
   cpu->state.pc += i->bytes;

   if (i->lua_before_handler != LUA_NOREF) {
      lua_rawgeti(cpu->lua->state, LUA_REGISTRYINDEX, i->lua_before_handler);
      ewm_lua_push_cpu(cpu->lua, cpu);
      lua_pushinteger(cpu->lua->state, i->opcode);
      switch (i->bytes) {
         case 1:
            lua_pushinteger(cpu->lua->state, 0);
            break;
         case 2:
            lua_pushinteger(cpu->lua->state, mem_get_byte(cpu, pc+1));
            break;
         case 3:
            lua_pushinteger(cpu->lua->state, mem_get_word(cpu, pc+1));
            break;
      }
      if (lua_pcall(cpu->lua->state, 3, 0, 0) != 0) {
         printf("cpu: script error: %s\n", lua_tostring(cpu->lua->state, -1));
      }
   }

   /* Execute instruction */
   switch (i->bytes) {
      case 1:
         ((cpu_instruction_handler_t) i->handler)(cpu);
         break;
      case 2:
         ((cpu_instruction_handler_byte_t) i->handler)(cpu, mem_get_byte(cpu, pc+1));
         break;
      case 3:
         ((cpu_instruction_handler_word_t) i->handler)(cpu, mem_get_word(cpu, pc+1));
         break;
   }

   if (i->lua_after_handler != LUA_NOREF) {
      lua_rawgeti(cpu->lua->state, LUA_REGISTRYINDEX, i->lua_after_handler);
      ewm_lua_push_cpu(cpu->lua, cpu);
      lua_pushinteger(cpu->lua->state, i->opcode);
      switch (i->bytes) {
         case 1:
            lua_pushinteger(cpu->lua->state, 0);
            break;
         case 2:
            lua_pushinteger(cpu->lua->state, mem_get_byte(cpu, pc+1));
            break;
         case 3:
            lua_pushinteger(cpu->lua->state, mem_get_word(cpu, pc+1));
            break;
      }
      if (lua_pcall(cpu->lua->state, 3, 0, 0) != 0) {
         printf("cpu: script error: %s\n", lua_tostring(cpu->lua->state, -1));
      }
   }

   cpu->counter += i->cycles;

   return i->cycles;
}

/* Public API */

static bool cpu_initialized = false;

static void cpu_initialize() {
   for (int i = 0; i <= 255; i++) {
      if (instructions_65C02[i].handler == NULL) {
         instructions_65C02[i] = instructions[i];
      }
   }
}

static int cpu_init(struct cpu_t *cpu, int model) {
   if (!cpu_initialized) {
      cpu_initialize();
      cpu_initialized = true;
   }

   memset(cpu, 0x00, sizeof(struct cpu_t));
   cpu->model = model;
   cpu->instructions = malloc(sizeof instructions);
   memcpy(cpu->instructions, (cpu->model == EWM_CPU_MODEL_6502) ? instructions : instructions_65C02, sizeof instructions);

   return 0;
}

struct cpu_t *cpu_create(int model) {
   struct cpu_t *cpu = malloc(sizeof(struct cpu_t));
   if (cpu_init(cpu, model) != 0) {
      cpu_destroy(cpu);
      free(cpu);
      cpu = NULL;
   }
   return cpu;
}

void cpu_destroy(struct cpu_t *cpu) {
   if (cpu->instructions != NULL) {
      free(cpu->instructions);
   }
   if (cpu->trace != NULL) {
      (void) fclose(cpu->trace);
      cpu->trace = NULL;
   }
}

static struct mem_t *cpu_mem_for_page(struct cpu_t *cpu, uint8_t page) {
   struct mem_t *mem = cpu->mem;
   while (mem != NULL) {
      if (mem->enabled && ((page * 0x100) >= mem->start) && ((page * 0x0100 + 0xff) <= mem->end)) {
         return mem;
      }
      mem = mem->next;
   }
   return NULL;
}

struct mem_t *cpu_add_mem(struct cpu_t *cpu, struct mem_t *mem) {
  if (cpu->mem == NULL) {
    cpu->mem = mem;
    mem->next = NULL;
  } else {
    mem->next = cpu->mem;
    cpu->mem = mem;
  }
  return mem;
}

// RAM Memory

static uint8_t _ram_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return ((uint8_t*) mem->obj)[addr - mem->start];
}

static void _ram_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
  ((uint8_t*) mem->obj)[addr - mem->start] = b;
}

struct mem_t *cpu_add_ram(struct cpu_t *cpu, uint16_t start, uint16_t end) {
   return cpu_add_ram_data(cpu, start, end, calloc(end-start+1, 0x01));
}

struct mem_t *cpu_add_ram_data(struct cpu_t *cpu, uint16_t start, uint16_t end, uint8_t *data) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  memset(mem, 0, sizeof(struct mem_t));
  mem->enabled = true;
  mem->flags = MEM_FLAGS_READ | MEM_FLAGS_WRITE;
  mem->obj = data;
  mem->start = start;
  mem->end = end;
  mem->read_handler = _ram_read;
  mem->write_handler = _ram_write;
  mem->next = NULL;
  return cpu_add_mem(cpu, mem);
}

struct mem_t *cpu_add_ram_file(struct cpu_t *cpu, uint16_t start, char *path) {
   int fd = open(path, O_RDONLY);
   if (fd == -1) {
      return NULL;
   }

   struct stat file_info;
   if (fstat(fd, &file_info) == -1) {
      close(fd);
      return NULL;
   }

   if (file_info.st_size  > (64 * 1024 - start)) {
      close(fd);
      return NULL;
   }

   char *data = calloc(file_info.st_size, 1);
   if (read(fd, data, file_info.st_size) != file_info.st_size) {
      close(fd);
      return NULL;
   }

   close(fd);

   return cpu_add_ram_data(cpu, start, start + file_info.st_size - 1, (uint8_t*) data);
}

// ROM Memory

static uint8_t _rom_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
  return ((uint8_t*) mem->obj)[addr - mem->start];
}

struct mem_t *cpu_add_rom_data(struct cpu_t *cpu, uint16_t start, uint16_t end, uint8_t *data) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  memset(mem, 0, sizeof(struct mem_t));
  mem->enabled = true;
  mem->flags = MEM_FLAGS_READ;
  mem->obj = data;
  mem->start = start;
  mem->end = end;
  mem->read_handler = _rom_read;
  mem->write_handler = NULL;
  mem->next = NULL;
  return cpu_add_mem(cpu, mem);
}

struct mem_t *cpu_add_rom_file(struct cpu_t *cpu, uint16_t start, char *path) {
   int fd = open(path, O_RDONLY);
   if (fd == -1) {
      return NULL;
   }

   struct stat file_info;
   if (fstat(fd, &file_info) == -1) {
      close(fd);
      return NULL;
   }

   if (file_info.st_size  > (64 * 1024 - start)) {
      close(fd);
      return NULL;
   }

   char *data = calloc(file_info.st_size, 1);
   if (read(fd, data, file_info.st_size) != file_info.st_size) {
      close(fd);
      return NULL;
   }

   close(fd);

   struct mem_t *result = cpu_add_rom_data(cpu, start, start + file_info.st_size - 1, (uint8_t*) data);
   result->description = path;
   return result;
}

// IO Memory

struct mem_t *cpu_add_iom(struct cpu_t *cpu, uint16_t start, uint16_t end, void *obj, mem_read_handler_t read_handler, mem_write_handler_t write_handler) {
  struct mem_t *mem = (struct mem_t*) malloc(sizeof(struct mem_t));
  memset(mem, 0, sizeof(struct mem_t));
  mem->enabled = true;
  mem->flags = MEM_FLAGS_READ | MEM_FLAGS_WRITE;
  mem->obj = obj;
  mem->start = start;
  mem->end = end;
  mem->read_handler = read_handler;
  mem->write_handler = write_handler;
  mem->next = NULL;
  return cpu_add_mem(cpu, mem);
}

// For now, as a good optimization, this emulator is going to assume
// that there is a memory region covering at least the first two pages
// of memory. This will probably break on the IIe where $0200 to $BFFF
// is also bank switched. But that is a problem for later.

void cpu_optimize_memory(struct cpu_t *cpu) {
   struct mem_t *zp = cpu_mem_for_page(cpu, 0);
   if (zp == NULL || (zp->flags != (MEM_FLAGS_READ | MEM_FLAGS_WRITE)) || zp->end < 0x01ff) {
      printf("[CPU] Cannot find a rw memory region that covers at least the first two pages\n");
      exit(1);
   }
   cpu->ram = zp->obj;
   cpu->ram_size = zp->end + 1;
}

void cpu_strict(struct cpu_t *cpu, bool strict) {
   cpu->strict = strict;
}

int cpu_trace(struct cpu_t *cpu, char *path) {
   if (cpu->trace != NULL) {
      (void) fclose(cpu->trace);
      cpu->trace = NULL;
   }

   if (path != NULL) {
      cpu->trace = fopen(path, "w");
      if (cpu->trace == NULL) {
         return errno;
      }
   }

   return 0;
}

void cpu_reset(struct cpu_t *cpu) {
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_RES);
   cpu->state.a = 0x00;
   cpu->state.x = 0x00;
   cpu->state.y = 0x00;
   cpu->state.n = 0;
   cpu->state.v = 0;
   cpu->state.b = 0;
   cpu->state.d = 0;
   cpu->state.i = 1;
   cpu->state.z = 0;
   cpu->state.c = 0;
   cpu->state.sp = 0xff;

   cpu_optimize_memory(cpu);
}

int cpu_irq(struct cpu_t *cpu) {
   if (cpu->strict && _cpu_stack_free(cpu) < 3) {
      return EWM_CPU_ERR_STACK_OVERFLOW;
   }

   _cpu_push_word(cpu, cpu->state.pc + 1); // TODO +1?? Spec says +2 but test fails then
   _cpu_push_byte(cpu, _cpu_get_status(cpu));
   cpu->state.i = 1;
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_IRQ);

   return 0;
}

int cpu_nmi(struct cpu_t *cpu) {
   if (cpu->strict && _cpu_stack_free(cpu) < 3) {
      return EWM_CPU_ERR_STACK_OVERFLOW;
   }

   _cpu_push_word(cpu, cpu->state.pc + 1); // TODO +1?? Spec says +2 but test fails then
   _cpu_push_byte(cpu, _cpu_get_status(cpu));
   cpu->state.i = 1;
   cpu->state.pc = mem_get_word(cpu, EWM_VECTOR_NMI);

   return 0;
}

int cpu_step(struct cpu_t *cpu) {
   return cpu_execute_instruction(cpu);
}

// Lua support

// cpu state functions

static int cpu_lua_index(lua_State *state) {
   void *cpu_data = luaL_checkudata(state, 1, "cpu_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);

   if (!lua_isstring(state, 2)) {
      printf("TODO lua_cpu_index: arg 2 is not a string\n");
      return 0;
   }

   const char *name = lua_tostring(state, 2);

   if (strcmp(name, "a") == 0) {
      lua_pushnumber(state, cpu->state.a);
      return 1;
   }

   if (strcmp(name, "x") == 0) {
      lua_pushnumber(state, cpu->state.x);
      return 1;
   }

   if (strcmp(name, "y") == 0) {
      lua_pushnumber(state, cpu->state.y);
      return 1;
   }

   if (strcmp(name, "s") == 0) {
      lua_pushnumber(state, _cpu_get_status(cpu));
      return 1;
   }

   if (strcmp(name, "pc") == 0) {
      lua_pushnumber(state, cpu->state.pc);
      return 1;
   }

   if (strcmp(name, "sp") == 0) {
      lua_pushnumber(state, cpu->state.sp);
      return 1;
   }

   if (strcmp(name, "model") == 0) {
      switch (cpu->model) {
         case EWM_CPU_MODEL_6502:
            lua_pushstring(state, "6502");
            break;
         case EWM_CPU_MODEL_65C02:
            lua_pushstring(state, "65C02");
            break;
      }
      return 1;
   }

   if (strcmp(name, "memory") == 0) {
      void *cpu_data = lua_newuserdata(state, sizeof(struct cpu_t*));
      *((struct cpu_t**) cpu_data) = cpu;
      luaL_getmetatable(state, "mem_meta_table");
      lua_setmetatable(state, -2);
      return 1;
   }

   luaL_getmetatable(state, "cpu_methods_meta_table");
   lua_pushvalue(state, 2);
   lua_rawget(state, -2);

   return 1;
}

static int cpu_lua_newindex(lua_State *state) {
   void *cpu_data = luaL_checkudata(state, 1, "cpu_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);

   if(!lua_isstring(state, 2)) {
      printf("TODO lua_cpu_new_index: arg 2 is not a string\n");
      return 0;
   }

   if(!lua_isnumber(state, 3)) {
      printf("TODO lua_cpu_new_index: arg 3 is not a string\n");

      return 0;
   }

   const char *name = lua_tostring(state, 2);
   int value = lua_tointeger(state, 3);

   if (strcmp(name, "a") == 0) {
      cpu->state.a = (uint8_t) value;
      return 0;
   }

   if (strcmp(name, "x") == 0) {
      cpu->state.x = (uint8_t) value;
      return 0;
   }

   if (strcmp(name, "y") == 0) {
      cpu->state.y = (uint8_t) value;
      return 0;
   }

   if (strcmp(name, "s") == 0) {
      _cpu_set_status(cpu, (uint8_t) value);
      return 0;
   }

   if (strcmp(name, "pc") == 0) {
      cpu->state.pc = (uint16_t) value;
      return 0;
   }

   if (strcmp(name, "sp") == 0) {
      cpu->state.pc = (uint16_t) value;
      return 0;
   }

   return 0;
}

// mem

static int cpu_lua_mem_index(lua_State *state) {
   void *cpu_data = luaL_checkudata(state, 1, "mem_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);

   if (lua_type(state, 2) != LUA_TNUMBER) {
      printf("First arg fail\n");
      return 0;
   }

   uint16_t addr = lua_tointeger(state, 2);
   lua_pushinteger(state, mem_get_byte(cpu, addr));

   return 1;
}

static int cpu_lua_mem_newindex(lua_State *state) {
   printf("mem_newindex()\n");

   void *cpu_data = luaL_checkudata(state, 1, "mem_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);

   if (lua_type(state, 2) != LUA_TNUMBER) {
      printf("First arg fail\n");
      return 0;
   }
   uint16_t addr = lua_tointeger(state, 2);

   if (lua_type(state, 3) != LUA_TNUMBER) {
      printf("First arg fail\n");
      return 0;
   }
   uint8_t value = lua_tointeger(state, 3);

   mem_set_byte(cpu, addr, value);

   return 0;
}

static int cpu_lua_reset(lua_State *state) {
   void *cpu_data = luaL_checkudata(state, 1, "cpu_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);
   cpu_reset(cpu);
   return 0;
}

// cpu module functions

// onBeforeExecution(op, fn)
static int cpu_lua_onBeforeExecuteInstruction(lua_State *state) {
   if (lua_gettop(state) != 3) {
      printf("Not enough arguments\n");
      return 0;
   }

   void *cpu_data = luaL_checkudata(state, 1, "cpu_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);

   if (lua_type(state, 2) != LUA_TNUMBER) {
      printf("First arg fail\n");
      return 0;
   }

   if (lua_type(state, 3) != LUA_TFUNCTION) {
      printf("Second arg fail\n");
      return 0;
   }

   uint8_t opcode = lua_tointeger(state, 2);

   lua_pushvalue(state, 3);
   cpu->instructions[opcode].lua_before_handler = luaL_ref(state, LUA_REGISTRYINDEX);

   return 0;
}

// onAfterExecuteFuncton(op, fn)
static int cpu_lua_onAfterExecuteInstruction(lua_State *state) {
   if (lua_gettop(state) != 3) {
      printf("Not enough arguments\n");
      return 0;
   }

   void *cpu_data = luaL_checkudata(state, 1, "cpu_meta_table");
   struct cpu_t *cpu = *((struct cpu_t**) cpu_data);

   if(lua_type(state, 2) != LUA_TNUMBER) {
      printf("First arg fail\n");
      return 0;
   }

   if(lua_type(state, 3) != LUA_TFUNCTION) {
      printf("Second arg fail\n");
      return 0;
   }

   uint8_t opcode = lua_tointeger(state, 2);

   lua_pushvalue(state, 3);
   cpu->instructions[opcode].lua_after_handler = luaL_ref(state, LUA_REGISTRYINDEX);

   return 0;
}

int ewm_cpu_init_lua(struct cpu_t *cpu, struct ewm_lua_t *lua) {
   // TODO Most of this needs to move to cpu_luaopen so that we don't
   // actually enable lua support until this module is required in a
   // script. Same for other components.

   cpu->lua = lua;

   luaL_Reg functions[] = {
      {"__index", cpu_lua_index},
      {"__newindex", cpu_lua_newindex},
      {NULL, NULL}
   };
   ewm_lua_register_component(lua, "cpu", functions);

   luaL_Reg cpu_methods[] = {
      {"onBeforeExecuteInstruction", cpu_lua_onBeforeExecuteInstruction},
      {"onAfterExecuteInstruction", cpu_lua_onAfterExecuteInstruction},
      {"reset", cpu_lua_reset},
      {NULL, NULL}
   };
   ewm_lua_register_component(lua, "cpu_methods", cpu_methods);

   // Register a global cpu instance

   void *cpu_data = lua_newuserdata(lua->state, sizeof(struct cpu_t*));
   *((struct cpu_t**) cpu_data) = cpu;

   luaL_getmetatable(lua->state, "cpu_meta_table");
   lua_setmetatable(lua->state, -2);
   lua_setglobal(lua->state, "cpu");

   // Register cpu.memory

   luaL_Reg mem_functions[] = {
      {"__index", cpu_lua_mem_index},
      {"__newindex", cpu_lua_mem_newindex},
      {NULL, NULL}
   };
   ewm_lua_register_component(lua, "mem", mem_functions);

   return 0;
}
