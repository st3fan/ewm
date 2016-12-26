
all: echo.bin textpages.bin lores.bin buttons.bin

clean:
	rm -f *.o *.bin *.lst *.hex

echo.bin: echo.o
	ld65 -t none -vm -o $@ $<
echo.o: echo.s
	ca65 -l $<

textpages.bin: textpages.o
	ld65 -t none -vm -o $@ $<
textpages.o: textpages.s
	ca65 -l $<

lores.bin: lores.o
	ld65 -t none -vm -o $@ $<
lores.o: lores.s
	ca65 -l $<

buttons.bin: buttons.o
	ld65 -t none -vm -o $@ $<
buttons.o: buttons.s
	ca65 -l $<
