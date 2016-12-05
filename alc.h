// The MIT License (MIT)
//
// Copyright (c) 2015 Stefan Arentz - http://github.com/st3fan/ewm
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#ifndef EWM_ALC_H
#define EWM_ALC_H

struct mem_t;
struct cpu_t;

struct ewm_alc_t {
   struct mem_t *ram1; // $D000 - $DFFF RAM Bank #1
   struct mem_t *ram2; // $D000 - $DFFF RAM Bank #2
   struct mem_t *ram3; // $E000 - $FFFF RAM Bank #3
   struct mem_t *rom;  // $F800 - $FFFF Autostart ROM
   struct mem_t *iom;  // $C080 - $C08F
   int wrtcount;
};

struct ewm_alc_t *ewm_alc_create(struct cpu_t *cpu);

#endif // EWM_ALC_H
