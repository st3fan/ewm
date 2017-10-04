# The MIT License (MIT)
#
# Copyright (c) 2015 Stefan Arentz - https://github.com/st3fan/ewm
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

UNAME := $(shell uname)

CC?=cc
CFLAGS=-std=gnu11 -Wall -Wextra -Werror -Wno-unused-parameter -I/usr/include
LDFLAGS=-L/usr/local/lib

ifndef DEBUG
  CFLAGS += -O3 -flto
  LDFLAGS += -flto
else
  CFLAGS += -Og -g
  LDFLAGS += -g
endif

ifdef LUA
  CFLAGS += -DEWM_LUA
  ifeq ($(UNAME), Linux)
    LUA_LIBS = -llua5.1
    CFLAGS += -I/usr/include/lua5.1
    LDFLAGS += -L/usr/lib/arm-linux-gnueabihf
  else
    LUA_LIBS=-llua
  endif
endif

CPU_SOURCES=cpu.c mem.c fmt.c ins.c utl.c
ifdef LUA
  CPU_SOURCES += lua.c
endif

EWM_EXECUTABLE=ewm
EWM_SOURCES=$(CPU_SOURCES) pia.c ewm.c two.c scr.c dsk.c chr.c alc.c one.c tty.c boo.c sdl.c
EWM_OBJECTS=$(EWM_SOURCES:.c=.o)
EWM_LIBS=-lSDL2 $(LUA_LIBS)

CPU_TEST_EXECUTABLE=cpu_test
CPU_TEST_SOURCES=$(CPU_SOURCES) cpu_test.c
CPU_TEST_OBJECTS=$(CPU_TEST_SOURCES:.c=.o)
CPU_TEST_LIBS=$(LUA_LIBS)

SCR_TEST_EXECUTABLE=scr_test
SCR_TEST_SOURCES=$(CPU_SOURCES) two.c scr.c dsk.c chr.c alc.c scr_test.c sdl.c
SCR_TEST_OBJECTS=$(SCR_TEST_SOURCES:.c=.o)
SCR_TEST_LIBS=-lSDL2 $(LUA_LIBS)

TTY_TEST_EXECUTABLE=tty_test
TTY_TEST_SOURCES=$(CPU_SOURCES) one.c tty.c pia.c chr.c tty_test.c sdl.c
TTY_TEST_OBJECTS=$(TTY_TEST_SOURCES:.c=.o)
TTY_TEST_LIBS=-lSDL2 $(LUA_LIBS)

CPU_BENCH=cpu_bench
CPU_BENCH_SOURCES=$(CPU_SOURCES) cpu_bench.c
CPU_BENCH_OBJECTS=$(CPU_BENCH_SOURCES:.c=.o)
CPU_BENCH_LIBS=$(LUA_LIBS)

MEM_BENCH=mem_bench
MEM_BENCH_SOURCES=$(CPU_SOURCES) mem_bench.c
MEM_BENCH_OBJECTS=$(MEM_BENCH_SOURCES:.c=.o)
MEM_BENCH_LIBS=$(LUA_LIBS)

all: $(EWM_SOURCES) $(EWM_EXECUTABLE) $(CPU_TEST_SOURCES) $(CPU_TEST_EXECUTABLE) $(SCR_TEST_EXECUTABLE) $(TTY_TEST_EXECUTABLE) $(CPU_BENCH) $(MEM_BENCH)

clean:
	rm -f $(EWM_OBJECTS) $(EWM_EXECUTABLE) $(CPU_TEST_OBJECTS) $(CPU_TEST_EXECUTABLE) $(SCR_TEST_OBJECTS) $(SCR_TEST_EXECUTABLE) $(TTY_TEST_EXECUTABLE) $(CPU_BENCH) $(MEM_BENCH)

$(EWM_EXECUTABLE): $(EWM_OBJECTS)
	$(CC) $(LDFLAGS) $(EWM_OBJECTS) $(EWM_LIBS) -o $@

$(CPU_TEST_EXECUTABLE): $(CPU_TEST_OBJECTS)
	$(CC) $(LDFLAGS) $(CPU_TEST_OBJECTS) $(CPU_TEST_LIBS) -o $@

$(SCR_TEST_EXECUTABLE): $(SCR_TEST_OBJECTS)
	$(CC) $(LDFLAGS) $(SCR_TEST_OBJECTS) $(SCR_TEST_LIBS) -o $@

$(TTY_TEST_EXECUTABLE): $(TTY_TEST_OBJECTS)
	$(CC) $(LDFLAGS) $(TTY_TEST_OBJECTS) $(TTY_TEST_LIBS) -o $@

$(CPU_BENCH): $(CPU_BENCH_OBJECTS)
	$(CC) $(LDFLAGS) $(CPU_BENCH_OBJECTS) $(CPU_BENCH_LIBS) -o $@

$(MEM_BENCH): $(MEM_BENCH_OBJECTS)
	$(CC) $(LDFLAGS) $(MEM_BENCH_OBJECTS) $(MEM_BENCH_LIBS) -o $@

.c.o:
	$(CC) $(CFLAGS) $< -c -o $@
