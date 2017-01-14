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

CC?=cc
CFLAGS=-std=gnu11 -O3 -Wall -Wextra -Werror -Wno-unused-parameter
LDFLAGS=-g -L/usr/local/lib

EWM_EXECUTABLE=ewm
EWM_SOURCES=cpu.c ins.c pia.c mem.c ewm.c fmt.c two.c scr.c dsk.c chr.c alc.c one.c tty.c utl.c
EWM_OBJECTS=$(EWM_SOURCES:.c=.o)
EWM_LIBS=-lSDL2

CPU_TEST_EXECUTABLE=cpu_test
CPU_TEST_SOURCES=cpu.c ins.c mem.c fmt.c cpu_test.c
CPU_TEST_OBJECTS=$(CPU_TEST_SOURCES:.c=.o)
CPU_TEST_LIBS=

SCR_TEST_EXECUTABLE=scr_test
SCR_TEST_SOURCES=cpu.c ins.c mem.c fmt.c two.c scr.c dsk.c chr.c alc.c utl.c scr_test.c
SCR_TEST_OBJECTS=$(SCR_TEST_SOURCES:.c=.o)
SCR_TEST_LIBS=-lSDL2

MEM_BENCH=mem_bench
MEM_BENCH_SOURCES=cpu.c ins.c mem.c fmt.c mem_bench.c
MEM_BENCH_OBJECTS=$(MEM_BENCH_SOURCES:.c=.o)
MEM_BENCH_LIBS=

all: $(EWM_SOURCES) $(EWM_EXECUTABLE) $(CPU_TEST_SOURCES) $(CPU_TEST_EXECUTABLE) $(SCR_TEST_EXECUTABLE) $(MEM_BENCH)

clean:
	rm -f $(EWM_OBJECTS) $(EWM_EXECUTABLE) $(CPU_TEST_OBJECTS) $(CPU_TEST_EXECUTABLE) $(SCR_TEST_OBJECTS) $(SCR_TEST_EXECUTABLE) $(MEM_BENCH)

$(EWM_EXECUTABLE): $(EWM_OBJECTS)
	$(CC) $(LDFLAGS) $(EWM_OBJECTS) $(EWM_LIBS) -o $@

$(CPU_TEST_EXECUTABLE): $(CPU_TEST_OBJECTS)
	$(CC) $(LDFLAGS) $(CPU_TEST_OBJECTS) $(CPU_TEST_LIBS) -o $@

$(SCR_TEST_EXECUTABLE): $(SCR_TEST_OBJECTS)
	$(CC) $(LDFLAGS) $(SCR_TEST_OBJECTS) $(SCR_TEST_LIBS) -o $@

$(MEM_BENCH): $(MEM_BENCH_OBJECTS)
	$(CC) $(LDFLAGS) $(MEM_BENCH_OBJECTS) $(MEM_BENCH_LIBS) -o $@

.c.o:
	$(CC) $(CFLAGS) $< -c -o $@
