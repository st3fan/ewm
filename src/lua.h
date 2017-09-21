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

#ifndef LUA_H
#define LUA_H

#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>

struct cpu_t;
struct ewm_two_t;
struct ewm_dsk_t;

struct ewm_lua_t {
   lua_State *state;
};

struct ewm_lua_t *ewm_lua_create();
int ewm_lua_load_script(struct ewm_lua_t *lua, char *script_path);

void ewm_lua_push_cpu(struct ewm_lua_t *lua, struct cpu_t *cpu);
void ewm_lua_push_two(struct ewm_lua_t *lua, struct ewm_two_t *two);
void ewm_lua_push_dsk(struct ewm_lua_t *lua, struct ewm_dsk_t *dsk);

void ewm_lua_register_component(struct ewm_lua_t *lua, char *name, luaL_Reg *functions);

void luaL_setfuncs (lua_State *L, const luaL_Reg *l, int nup);

#endif // LUA_H
