
        .ORG $8000

KBD     = $D010                 ;  PIA.A keyboard input
KBDCR   = $D011                 ;  PIA.A keyboard control register
DSP     = $D012                 ;  PIA.B display output register
DSPCR   = $D013                 ;  PIA.B display control register

MAIN:   JSR GETC                ;
        JSR PUTC                ;

        CMP #$03                ;
        BNE MAIN                ;
        BRK                     ;

GETC:   LDA KBDCR               ;
        BPL GETC                ; Do we have a character?
        LDA KBD                 ; Yup, load it
        RTS                     ;

PUTC:   BIT DSP                 ; Bit (B7) cleared yet?
        BMI PUTC                ; No, wait for display
        STA DSP                 ; Yup, send it to the display
        RTS                     ;
