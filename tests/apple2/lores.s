
        .org    $8000

ptr		= $06

full            = $c052
mixed           = $c053

page1   	= $c054
page2		= $c055

kybd		= $c000
strobe		= $c010


main:   lda $c054
        lda $c056
        lda $c052
        lda $c050

        jsr clear1
        jsr clear2

loop:   lda page1               ; mixed page 1
        lda mixed
        jsr clear1
        jsr settext1
        jsr wait

        lda page1               ;full page 1
        lda full
        jsr clear1
        jsr wait

        lda page2               ;mixed page 2
        lda mixed
        jsr clear2
        jsr settext2
        jsr wait

        lda page2               ;full page 2
        lda full
        jsr clear2
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
start1: lda #$11
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
start2: lda #$22
loop2:  sta (ptr),y
        iny
        bne loop2
nxt2:   inc ptr+1
        lda ptr+1
        cmp #$0c
        bcc start2
        rts


settext1:  lda #$06
        sta ptr+1
        ldy #$50
        sty ptr
xstart2: lda #$B1
xloop2:  sta (ptr),y
        iny
        bne xloop2
xnxt2:   inc ptr+1
        lda ptr+1
        cmp #$08
        bcc xstart2
        rts


settext2:  lda #$0a
        sta ptr+1
        ldy #$50
        sty ptr
ystart2:        lda #$B2
yloop2:  sta (ptr),y
        iny
        bne yloop2
ynxt2:   inc ptr+1
        lda ptr+1
        cmp #$0c
        bcc ystart2
        rts
