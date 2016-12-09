
       	.org	$8000

cout	= $fded
home	= $fc58

pb0     = $c061
pb1     = $c062
pb2     = $c063
pb3     = $c060

        jsr home

check0: lda pb0
        beq check1
        lda #'0'
        jsr cout

check1: lda pb1
        beq check2
        lda #'1'
        jsr cout

check2: lda pb2
        beq check3
        lda #'2'
        jsr cout

check3: lda pb3
        beq check0
        lda #'3'
        jsr cout

        jmp check0

