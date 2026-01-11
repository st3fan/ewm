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

#ifndef EWM_SND_H
#define EWM_SND_H

#include <stdint.h>
#include <SDL2/SDL.h>

#define EWM_SND_SAMPLE_RATE (44100)
#define EWM_SND_BUFFER_SIZE (4096)

struct ewm_two_t;

struct ewm_snd_t {
   struct ewm_two_t *two;
   SDL_AudioDeviceID device;
   int speaker_state;
   uint64_t last_toggle_cycle;
   int16_t *buffer;
   int buffer_index;
   uint64_t cycles_per_frame;
   uint64_t frame_start_cycle;
};

struct ewm_snd_t *ewm_snd_create(struct ewm_two_t *two);
void ewm_snd_destroy(struct ewm_snd_t *snd);
void ewm_snd_toggle_speaker(struct ewm_snd_t *snd, uint64_t cpu_counter);
void ewm_snd_update(struct ewm_snd_t *snd, uint64_t cpu_counter);

#endif
