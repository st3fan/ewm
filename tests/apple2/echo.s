
	.org	$8000

kybd	= $c000
strobe	= $c010
cout	= $fded
home	= $fc58

start:	jsr home
loop:	lda kybd
	cmp #$80
	bcc loop
	sta strobe
	jsr cout
	jmp loop

