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
#include <string.h>

#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>

#include "lua.h"
#include "two.h"
#include "cpu.h"
#include "dsk.h"

static int ewm_lua_init(struct ewm_lua_t *lua) {
   memset(lua, 0x00, sizeof(struct ewm_lua_t));
   lua->state = luaL_newstate();
   luaL_openlibs(lua->state);
   return 0;
}

struct ewm_lua_t *ewm_lua_create() {
   struct ewm_lua_t *lua = malloc(sizeof(struct ewm_lua_t));
   if (ewm_lua_init(lua) != 0) {
      free(lua);
      lua = NULL;
   }
   return lua;
}

int ewm_lua_load_script(struct ewm_lua_t *lua, char *script_path) {
   if (luaL_dofile(lua->state, script_path) != LUA_OK) {
      printf("ewm: script error: %s\n", lua_tostring(lua->state, -1));
      return -1;
   }
   return 0;
}

void ewm_lua_push_cpu(struct ewm_lua_t *lua, struct cpu_t *cpu) {
   void *cpu_data = lua_newuserdata(lua->state, sizeof(struct cpu_t*));
   *((struct cpu_t**) cpu_data) = cpu;
   luaL_getmetatable(lua->state, "cpu_meta_table");
   lua_setmetatable(lua->state, -2);
}

void ewm_lua_push_two(struct ewm_lua_t *lua, struct ewm_two_t *two) {
   void *two_data = lua_newuserdata(lua->state, sizeof(struct ewm_two_t*));
   *((struct ewm_two_t**) two_data) = two;
   luaL_getmetatable(lua->state, "two_meta_table");
   lua_setmetatable(lua->state, -2);
}

void ewm_lua_push_dsk(struct ewm_lua_t *lua, struct ewm_dsk_t *dsk) {
   void *dsk_data = lua_newuserdata(lua->state, sizeof(struct ewm_dsk_t*));
   *((struct ewm_dsk_t**) dsk_data) = dsk;
   luaL_getmetatable(lua->state, "dsk_meta_table");
   lua_setmetatable(lua->state, -2);
}

void ewm_lua_register_component(struct ewm_lua_t *lua, char *name, luaL_Reg *functions) {
   char table_name[64];
   strncpy(table_name, name, sizeof(table_name)-1);
   strncat(table_name, "_meta_table", sizeof(table_name)-1);

   char global_name[64];
   strncpy(global_name, name, sizeof(global_name)-1);
   for (size_t i = 0; i < strlen(global_name); i++) {
      global_name[i] = toupper(global_name[i]);
   }

   // Register the cpu meta table
   luaL_newmetatable(lua->state, table_name);
   luaL_setfuncs(lua->state, functions, 0);
}
