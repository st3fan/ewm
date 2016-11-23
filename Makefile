# The MIT License (MIT)
#
# Copyright (c) 2015 Stefan Arentz - http:#github.com/st3fan/ewm
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

CC=cc
CFLAGS=-std=c11 -O2 -Wpedantic -Wall -Wshadow -Werror -Wshadow  -Wno-gnu-binary-literal -g
LDFLAGS=-g

EWM_EXECUTABLE=ewm
EWM_SOURCES=cpu.c ins.c pia.c mem.c ewm.c fmt.c
EWM_OBJECTS=$(EWM_SOURCES:.c=.o)
EWM_LIBS=-lcurses

CPU_TEST_EXECUTABLE=cpu_test
CPU_TEST_SOURCES=cpu.c ins.c mem.c fmt.c cpu_test.c
CPU_TEST_OBJECTS=$(CPU_TEST_SOURCES:.c=.o)
CPU_TEST_LIBS=

all: $(EWM_SOURCES) $(EWM_EXECUTABLE) $(CPU_TEST_SOURCES) $(CPU_TEST_EXECUTABLE)

clean:
	rm -f $(EWM_OBJECTS) $(EWM_EXECUTABLE) $(CPU_TEST_OBJECTS) $(CPU_TEST_EXECUTABLE)

$(EWM_EXECUTABLE): $(EWM_OBJECTS)
	$(CC) $(LDFLAGS) $(EWM_OBJECTS) $(EWM_LIBS) -o $@

$(CPU_TEST_EXECUTABLE): $(CPU_TEST_OBJECTS)
	$(CC) $(LDFLAGS) $(CPU_TEST_OBJECTS) $(CPU_TEST_LIBS) -o $@

.c.o:
	$(CC) $(CFLAGS) $< -c -o $@
