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

#include <stdlib.h>
#include <string.h>

#include "cpu.h"
#include "two.h"
#include "snd.h"

#define EWM_SND_AMPLITUDE (8000)
#define EWM_SND_CPU_FREQUENCY (1023000)

static int ewm_snd_init(struct ewm_snd_t *snd, struct ewm_two_t *two) {
   memset(snd, 0, sizeof(struct ewm_snd_t));
   snd->two = two;

   SDL_AudioSpec want, have;
   memset(&want, 0, sizeof(want));
   want.freq = EWM_SND_SAMPLE_RATE;
   want.format = AUDIO_S16SYS;
   want.channels = 1;
   want.samples = 512;
   want.callback = NULL;

   snd->device = SDL_OpenAudioDevice(NULL, 0, &want, &have, 0);
   if (snd->device == 0) {
      fprintf(stderr, "[SND] Failed to open audio device: %s\n", SDL_GetError());
      return -1;
   }

   snd->buffer = malloc(EWM_SND_BUFFER_SIZE * sizeof(int16_t));
   if (snd->buffer == NULL) {
      fprintf(stderr, "[SND] Failed to allocate audio buffer\n");
      SDL_CloseAudioDevice(snd->device);
      return -1;
   }

   snd->speaker_state = 0;
   snd->last_toggle_cycle = 0;
   snd->buffer_index = 0;
   snd->frame_start_cycle = 0;

   SDL_PauseAudioDevice(snd->device, 0);

   return 0;
}

struct ewm_snd_t *ewm_snd_create(struct ewm_two_t *two) {
   struct ewm_snd_t *snd = malloc(sizeof(struct ewm_snd_t));
   if (snd == NULL) {
      return NULL;
   }
   if (ewm_snd_init(snd, two) != 0) {
      free(snd);
      return NULL;
   }
   return snd;
}

void ewm_snd_destroy(struct ewm_snd_t *snd) {
   if (snd != NULL) {
      if (snd->device != 0) {
         SDL_CloseAudioDevice(snd->device);
      }
      if (snd->buffer != NULL) {
         free(snd->buffer);
      }
      free(snd);
   }
}

void ewm_snd_toggle_speaker(struct ewm_snd_t *snd, uint64_t cpu_counter) {
   if (snd == NULL) {
      return;
   }

   uint64_t cycles_since_frame_start = cpu_counter - snd->frame_start_cycle;
   int sample_index = (cycles_since_frame_start * EWM_SND_SAMPLE_RATE) / EWM_SND_CPU_FREQUENCY;

   int16_t amplitude = snd->speaker_state ? EWM_SND_AMPLITUDE : -EWM_SND_AMPLITUDE;
   while (snd->buffer_index < sample_index && snd->buffer_index < EWM_SND_BUFFER_SIZE) {
      snd->buffer[snd->buffer_index++] = amplitude;
   }

   snd->speaker_state = !snd->speaker_state;
   snd->last_toggle_cycle = cpu_counter;
}

void ewm_snd_update(struct ewm_snd_t *snd, uint64_t cpu_counter) {
   if (snd == NULL) {
      return;
   }

   uint64_t cycles_this_frame = cpu_counter - snd->frame_start_cycle;
   int samples_needed = (cycles_this_frame * EWM_SND_SAMPLE_RATE) / EWM_SND_CPU_FREQUENCY;

   if (samples_needed > EWM_SND_BUFFER_SIZE) {
      samples_needed = EWM_SND_BUFFER_SIZE;
   }

   int16_t amplitude = snd->speaker_state ? EWM_SND_AMPLITUDE : -EWM_SND_AMPLITUDE;
   while (snd->buffer_index < samples_needed) {
      snd->buffer[snd->buffer_index++] = amplitude;
   }

   if (snd->buffer_index > 0) {
      uint32_t queued = SDL_GetQueuedAudioSize(snd->device);
      if (queued < EWM_SND_SAMPLE_RATE * sizeof(int16_t) / 10) {
         SDL_QueueAudio(snd->device, snd->buffer, snd->buffer_index * sizeof(int16_t));
      }
   }

   snd->buffer_index = 0;
   snd->frame_start_cycle = cpu_counter;
}
