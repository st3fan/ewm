
all: echo.bin

clean:
	rm -f *.o *.bin *.lst *.hex

echo.bin: echo.o
	ld65 -t none -vm -o $@ $<
echo.o: echo.s
	ca65 -l $<

