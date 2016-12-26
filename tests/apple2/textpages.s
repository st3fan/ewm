
        .org    $8000

ptr		= $06

page2off	= $c054
page2on		= $c055
kybd		= $c000
strobe		= $c010


main:   jsr clear1
        jsr clear2
loop:   lda page2off
        jsr wait
        lda page2on
        jsr wait
        jmp loop


wait:   lda kybd
        cmp #$80
        bcc wait
        sta strobe
        rts


clear1:  lda #$04
        sta ptr+1
        ldy #$00
        sty ptr
start1: lda #'1'
loop1:  sta (ptr),y
        iny
        bne loop1
nxt1:   inc ptr+1
        lda ptr+1
        cmp #$08
        bcc start1
        rts


clear2:  lda #$08
        sta ptr+1
        ldy #$00
        sty ptr
start2: lda #'2'
loop2:  sta (ptr),y
        iny
        bne loop2
nxt2:   inc ptr+1
        lda ptr+1
        cmp #$0c
        bcc start2
        rts

