
CC=cc
CFLAGS=-O3 -std=c11 -Werror -Wall -Wshadow -Wpointer-arith -Wcast-qual -Wstrict-prototypes -Wmissing-prototypes
SOURCES=cpu.c ins.c pia.c mem.c ewm.c
OBJECTS=$(SOURCES:.c=.o)
LIBS=-lcurses
EXECUTABLE=ewm

all: $(SOURCES) $(EXECUTABLE)

clean:
	rm -f $(OBJECTS) $(EXECUTABLE)

$(EXECUTABLE): $(OBJECTS)
	$(CC) $(LDFLAGS) $(OBJECTS) $(LIBS) -o $@

.c.o:
	$(CC) $(CFLAGS) $< -c -o $@
