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

#include <assert.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>

#include "mem.h"
#include "cpu.h"
#include "utl.h"
#include "dsk.h"

//
// This implements a 16-sector Disk ][ controller with two drives
// attached. It is currently fixed to slot 6, which is pretty
// normal. That will be changed at a later stage when we introduce
// slots and cards in a more generic way.
//
// Most of this code is based on Beneath Apple DOS and another open
// source emulator at https://github.com/whscullin/apple2js
//

// Private

#define EWM_DISKII_PHASE0OFF 0xc0e0
#define EWM_DISKII_PHASE0ON  0xc0e1
#define EWM_DISKII_PHASE1OFF 0xc0e2
#define EWM_DISKII_PHASE1ON  0xc0e3
#define EWM_DISKII_PHASE2OFF 0xc0e4
#define EWM_DISKII_PHASE2ON  0xc0e5
#define EWM_DISKII_PHASE3OFF 0xc0e6
#define EWM_DISKII_PHASE3ON  0xc0e7

#define EWM_DISKII_DRIVEOFF  0xc0e8
#define EWM_DISKII_DRIVEON   0xc0e9
#define EWM_DISKII_DRIVE1    0xc0ea
#define EWM_DISKII_DRIVE2    0xc0eb
#define EWM_DISKII_READ      0xc0ec
#define EWM_DISKII_WRITE     0xc0ed
#define EWM_DISKII_READMODE  0xc0ee
#define EWM_DISKII_WRITEMODE 0xc0ef

#define EWM_DSK_MODE_READ 0
#define EWM_DSK_MODE_WRITE 1

static uint8_t dsk_rom[] = {
   0xa2,0x20,0xa0,0x00,0xa2,0x03,0x86,0x3c,0x8a,0x0a,0x24,0x3c,0xf0,0x10,0x05,0x3c,
   0x49,0xff,0x29,0x7e,0xb0,0x08,0x4a,0xd0,0xfb,0x98,0x9d,0x56,0x03,0xc8,0xe8,0x10,
   0xe5,0x20,0x58,0xff,0xba,0xbd,0x00,0x01,0x0a,0x0a,0x0a,0x0a,0x85,0x2b,0xaa,0xbd,
   0x8e,0xc0,0xbd,0x8c,0xc0,0xbd,0x8a,0xc0,0xbd,0x89,0xc0,0xa0,0x50,0xbd,0x80,0xc0,
   0x98,0x29,0x03,0x0a,0x05,0x2b,0xaa,0xbd,0x81,0xc0,0xa9,0x56,0x20,0xa8,0xfc,0x88,
   0x10,0xeb,0x85,0x26,0x85,0x3d,0x85,0x41,0xa9,0x08,0x85,0x27,0x18,0x08,0xbd,0x8c,
   0xc0,0x10,0xfb,0x49,0xd5,0xd0,0xf7,0xbd,0x8c,0xc0,0x10,0xfb,0xc9,0xaa,0xd0,0xf3,
   0xea,0xbd,0x8c,0xc0,0x10,0xfb,0xc9,0x96,0xf0,0x09,0x28,0x90,0xdf,0x49,0xad,0xf0,
   0x25,0xd0,0xd9,0xa0,0x03,0x85,0x40,0xbd,0x8c,0xc0,0x10,0xfb,0x2a,0x85,0x3c,0xbd,
   0x8c,0xc0,0x10,0xfb,0x25,0x3c,0x88,0xd0,0xec,0x28,0xc5,0x3d,0xd0,0xbe,0xa5,0x40,
   0xc5,0x41,0xd0,0xb8,0xb0,0xb7,0xa0,0x56,0x84,0x3c,0xbc,0x8c,0xc0,0x10,0xfb,0x59,
   0xd6,0x02,0xa4,0x3c,0x88,0x99,0x00,0x03,0xd0,0xee,0x84,0x3c,0xbc,0x8c,0xc0,0x10,
   0xfb,0x59,0xd6,0x02,0xa4,0x3c,0x91,0x26,0xc8,0xd0,0xef,0xbc,0x8c,0xc0,0x10,0xfb,
   0x59,0xd6,0x02,0xd0,0x87,0xa0,0x00,0xa2,0x56,0xca,0x30,0xfb,0xb1,0x26,0x5e,0x00,
   0x03,0x2a,0x5e,0x00,0x03,0x2a,0x91,0x26,0xc8,0xd0,0xee,0xe6,0x27,0xe6,0x3d,0xa5,
   0x3d,0xcd,0x00,0x08,0xa6,0x2b,0x90,0xdb,0x4c,0x01,0x08,0x00,0x00,0x00,0x00,0x00
};

// See Beneath Apple DOS 3-21
static uint8_t dsk_wr_table[] = {
   0x96, 0x97, 0x9a, 0x9b, 0x9d, 0x9e, 0x9f, 0xa6,
   0xa7, 0xab, 0xac, 0xad, 0xae, 0xaf, 0xb2, 0xb3,
   0xb4, 0xb5, 0xb6, 0xb7, 0xb9, 0xba, 0xbb, 0xbc,
   0xbd, 0xbe, 0xbf, 0xcb, 0xcd, 0xce, 0xcf, 0xd3,
   0xd6, 0xd7, 0xd9, 0xda, 0xdb, 0xdc, 0xdd, 0xde,
   0xdf, 0xe5, 0xe6, 0xe7, 0xe9, 0xea, 0xeb, 0xec,
   0xed, 0xee, 0xef, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6,
   0xf7, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff
};

static int dsk_phase_delta[4][4] = {
   { 0, 1, 2,-1},
   {-1, 0, 1, 2},
   {-2,-1, 0, 1},
   { 1,-2,-1, 0}
};

static struct ewm_dsk_drive_t *dsk_drive(struct ewm_dsk_t *dsk) {
   return &dsk->drives[dsk->drive];
}

static void dsk_phase(struct ewm_dsk_t *dsk, int phase, bool on) {
   if (on) {
      //printf("[DSK] Disk #%d phase %d on\n", dsk->drive, phase);
      struct ewm_dsk_drive_t *drive = dsk_drive(dsk);

      drive->track += dsk_phase_delta[drive->phase][phase];
      drive->phase = phase;

      if (drive->track > EWM_DSK_TRACKS * 2 - 1) {
         drive->track = EWM_DSK_TRACKS * 2 - 1;
      }

      if (drive->track < 0) {
         drive->track = 0;
      }

      //printf("[DSK]     Disk #%d track = %d\n", dsk->drive, drive->track);
   } else {
      //printf("[DSK] Disk #%d phase %d off\n", dsk->drive, phase);
   }
}

static void dsk_write_next(struct ewm_dsk_t *dsk, uint8_t v) {
   if (dsk->mode == EWM_DSK_MODE_WRITE) {
      dsk->latch = v;
   }
}

static uint8_t dsk_read_next(struct ewm_dsk_t *dsk) {
   uint8_t result = 0;
   if (dsk->skip || dsk->mode == EWM_DSK_MODE_WRITE) {
      struct ewm_dsk_drive_t *drive = dsk_drive(dsk);
      struct ewm_dsk_track_t track = drive->tracks[drive->track >> 1]; // TODO Because drv->track actually goes to 70?

      //printf("Reading track.data[%d] (track.length = %zu): %.2X\n", drive->head, track.length, track.data[drive->head]);

      if (drive->head >= track.length) {
         drive->head = 0;
      }

      if (dsk->mode == EWM_DSK_MODE_WRITE) {
         track.data[drive->head] = dsk->latch; // TODO Implement write support
      } else {
         result = track.data[drive->head];
      }

      drive->head += 1;
   }

   dsk->skip += 1;
   dsk->skip %= 4;

   return result;
}

static uint8_t dsk_read(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr) {
   //printf("[DSK] dsk_read at $%.4X\n", addr);
   struct ewm_dsk_t *dsk = (struct ewm_dsk_t*) mem->obj;
   uint8_t result = 0x00;

   switch (addr) {
      case EWM_DISKII_PHASE0OFF:
         dsk_phase(dsk, 0, false);
         break;
      case EWM_DISKII_PHASE0ON:
         dsk_phase(dsk, 0, true);
         break;
      case EWM_DISKII_PHASE1OFF:
         dsk_phase(dsk, 1, false);
         break;
      case EWM_DISKII_PHASE1ON:
         dsk_phase(dsk, 1, true);
         break;
      case EWM_DISKII_PHASE2OFF:
         dsk_phase(dsk, 2, false);
         break;
      case EWM_DISKII_PHASE2ON:
         dsk_phase(dsk, 2, true);
         break;
      case EWM_DISKII_PHASE3OFF:
         dsk_phase(dsk, 3, false);
         break;
      case EWM_DISKII_PHASE3ON:
         dsk_phase(dsk, 3, true);
         break;

      case EWM_DISKII_DRIVEOFF:
         //printf("[DSK] Drive #%d off\n", dsk->drive);
         dsk->on = false;
         // TODO Drive light
         break;
      case EWM_DISKII_DRIVEON:
         //printf("[DSK] Drive #%d on\n", dsk->drive);
         dsk->on = true;
         // TODO Drive light
         break;

      case EWM_DISKII_DRIVE1:
         //printf("[DSK] Select drive #%d\n", dsk->drive);
         dsk->drive = EWM_DSK_DRIVE1;
         // TODO Drive light
         break;
      case EWM_DISKII_DRIVE2:
         //printf("[DSK] Select drive #%d\n", dsk->drive);
         dsk->drive = EWM_DSK_DRIVE2;
         // TODO Drive light
         break;

      case EWM_DISKII_READMODE:
         dsk->mode = EWM_DSK_MODE_READ;
         if (dsk_drive(dsk)->loaded) {
            result = (dsk_read_next(dsk) & 0x7f) | (dsk_drive(dsk)->readonly ? 0x80 : 0x00);
         }
         break;
      case EWM_DISKII_WRITEMODE:
         dsk->mode = EWM_DSK_MODE_WRITE;
         break;

      case EWM_DISKII_READ:
         if (dsk_drive(dsk)->loaded) {
            result = dsk_read_next(dsk);
         }
         break;
      case EWM_DISKII_WRITE:
         // Called by code, but doesn't do anything?
         break;

      default:
         fprintf(stderr, "[DSK] Got an unhandled read from $%.4X\n", addr);
         break;
   }

   return result;
}

static void dsk_write(struct cpu_t *cpu, struct mem_t *mem, uint16_t addr, uint8_t b) {
   //printf("[DSK] dsk_write at $%.4X\n", addr);

   // TODO It is entirely possible that we need to handle to the exact same soft switches as in read
   struct ewm_dsk_t *dsk = (struct ewm_dsk_t*) mem->obj;
   switch (addr) {
      case EWM_DISKII_WRITE:
         dsk_write_next(dsk, b);
         break;
      case EWM_DISKII_WRITEMODE:
         dsk->mode = EWM_DSK_MODE_WRITE;
         break;
      default:
         fprintf(stderr, "[DSK] Got an unhandled write to $%.4X\n", addr);
         break;
   }
}

static int dsk_native_track_length(int track_idx) {
   int length = 0;
   for (int sector_idx = 0; sector_idx < EWM_DSK_SECTORS; sector_idx++) {
      // Gap 1
      if (sector_idx == 0) {
         length += 0x80;
      } else {
         if (track_idx == 0) {
            length += 0x28;
         } else {
            length += 0x26;
         }
      }
      // Address field
      length += 14;
      // Gap 2
      length += 5;
      // Data field
      length += 3 + 342 + 1 + 3;
      // Gap 3
      length += 1;
   }
   return length;
}

static uint8_t dsk_fourxfour_hi(uint8_t v) {
   return ((v & 0b10101010) >> 1) | 0b10101010;
}

static uint8_t dsk_fourxfour_lo(uint8_t v) {
   return (v & 0b01010101) | 0b10101010;
}

static uint8_t dsk_sector_ordering_do[EWM_DSK_SECTORS] = {
   0x00,0x0d,0x0b,0x09,0x07,0x05,0x03,0x01,0x0e,0x0c,0x0a,0x08,0x06,0x04,0x02,0x0f
};

static uint8_t dsk_sector_ordering_po[EWM_DSK_SECTORS] = {
   0x00,0x02,0x04,0x06,0x08,0x0a,0x0c,0x0e,0x01,0x03,0x05,0x07,0x09,0x0b,0x0d,0x0f
};

static uint8_t *dsk_convert_sector(struct ewm_dsk_t *dsk, struct ewm_dsk_drive_t *drive, int track_idx, int sector_idx, uint8_t *src, uint8_t *dst) {
   // Gap 1
   if (sector_idx == 0) {
      for (int i = 0; i < 0x80; i++) {
         *dst++ = 0xff;
      }
   } else {
      if (track_idx == 0) {
         for (int i = 0; i < 0x28; i++) {
            *dst++ = 0xff;
         }
      } else {
         for (int i = 0; i < 0x26; i++) {
            *dst++ = 0xff;
         }
      }
   }

   // Address Field
   uint8_t checksum = drive->volume ^ track_idx ^ sector_idx;
   *dst++ = 0xd5;
   *dst++ = 0xaa;
   *dst++ = 0x96;
   *dst++ = dsk_fourxfour_hi(drive->volume);
   *dst++ = dsk_fourxfour_lo(drive->volume);
   *dst++ = dsk_fourxfour_hi(track_idx);
   *dst++ = dsk_fourxfour_lo(track_idx);
   *dst++ = dsk_fourxfour_hi(sector_idx);
   *dst++ = dsk_fourxfour_lo(sector_idx);
   *dst++ = dsk_fourxfour_hi(checksum);
   *dst++ = dsk_fourxfour_lo(checksum);
   *dst++ = 0xde;
   *dst++ = 0xaa;
   *dst++ = 0xeb;

   // Gap 2
   for (int i = 0; i < 5; i++) {
      *dst++ = 0xff;
   }

   // Data Field
   *dst++ = 0xd5;
   *dst++ = 0xaa;
   *dst++ = 0xad;

   uint8_t nibbles[0x156];
   uint8_t ptr2 = 0;
   uint8_t ptr6 = 0x56;

   for (int i = 0; i < 0x156; i++) {
      nibbles[i] = 0;
   }

   int idx2 = 0x55;
   for (int idx6 = 0x101; idx6 >= 0; idx6--) {
      uint8_t val6 = src[idx6 % 0x100]; // TODO % 0x100 makes no sense on an uint8_t
      uint8_t val2 = nibbles[ptr2 + idx2];

      val2 = (val2 << 1) | (val6 & 1);
      val6 >>= 1;
      val2 = (val2 << 1) | (val6 & 1);
      val6 >>= 1;

      nibbles[ptr6 + idx6] = val6;
      nibbles[ptr2 + idx2] = val2;

      if (--idx2 < 0) {
         idx2 = 0x55;
      }
   }

   uint8_t last = 0;
   for (int i = 0; i < 0x156; i++) {
      uint8_t val = nibbles[i];
      *dst++ = dsk_wr_table[last ^ val];
      last = val;
   }
   *dst++ = dsk_wr_table[last];

   *dst++ = 0xde;
   *dst++ = 0xaa;
   *dst++ = 0xeb;

   // Gap 3
   *dst++ = 0xff;

   return dst;
}

static struct ewm_dsk_track_t dsk_convert_track(struct ewm_dsk_t *disk, struct ewm_dsk_drive_t *drive, uint8_t *data, int track_idx, int type) {
   struct ewm_dsk_track_t track;
   track.length = dsk_native_track_length(track_idx);
   track.data = malloc(track.length);

   uint8_t *sector_ordering = (type == EWM_DSK_TYPE_DO) ? dsk_sector_ordering_do : dsk_sector_ordering_po;

   uint8_t *dst = track.data;
   for (int sector_idx = 0; sector_idx < EWM_DSK_SECTORS; sector_idx++) {
      int _s = 15 - sector_idx;
      uint8_t *src = data
         + (track_idx * EWM_DSK_SECTORS * EWM_DSK_SECTOR_SIZE) // Start of track_idx
         + (_s * EWM_DSK_SECTOR_SIZE);    // Start of sector_idx
      dst = dsk_convert_sector(disk, drive, track_idx, sector_ordering[_s], src, dst);
   }

   return track;
}

// Disk file parsing

// Public

static int ewm_dsk_init(struct ewm_dsk_t *dsk, struct cpu_t *cpu) {
   memset(dsk, 0x00, sizeof(struct ewm_dsk_t));
   dsk->rom = cpu_add_rom_data(cpu, 0xc600, 0xc6ff, dsk_rom);
   dsk->rom->description = "rom/dsk/$C600";
   dsk->iom = cpu_add_iom(cpu, 0xc0e0, 0xc0ef, dsk, dsk_read, dsk_write);
   dsk->rom->description = "iom/dsk/$C0E0";
   return 0;
}

struct ewm_dsk_t *ewm_dsk_create(struct cpu_t *cpu) {
   struct ewm_dsk_t *dsk = (struct ewm_dsk_t*) malloc(sizeof(struct ewm_dsk_t));
   ewm_dsk_init(dsk, cpu);
   return dsk;
}

int ewm_dsk_set_disk_data(struct ewm_dsk_t *dsk, uint8_t index, bool readonly, void *data, size_t length, int type) {
   if (type == EWM_DSK_TYPE_UNKNOWN) {
      return -1;
   }

   if (index > 1) {
      return -1;
   }

   if (type == EWM_DSK_TYPE_DO || type == EWM_DSK_TYPE_PO) {
      if (length != (EWM_DSK_TRACKS * EWM_DSK_SECTORS * 256)) {
         return -1;
      }
   } else if (type == EWM_DSK_TYPE_NIB) {
      if (length != (EWM_DSK_TRACKS * EWM_DSK_NIBBLES_PER_TRACK)) {
         return -1;
      }
   }

   struct ewm_dsk_drive_t *drive = &dsk->drives[index];

   for (int t = 0; t < EWM_DSK_TRACKS; t++) {
      if (drive->tracks[t].data != NULL) {
         free(drive->tracks[t].data);
         drive->tracks[t].data = NULL;
         drive->tracks[t].length = 0;
      }
   }

   drive->loaded = true;
   drive->volume = 254; // TODO Find Volume from disk image. Or does this not matter? I guess this gets lost in .dsk files.
   drive->track = 0;
   drive->head = 0;
   drive->phase = 0;
   drive->readonly = readonly;
   drive->dirty = false;

   if (type == EWM_DSK_TYPE_DO || type == EWM_DSK_TYPE_PO) {
      for (int t = 0; t < EWM_DSK_TRACKS; t++) {
         drive->tracks[t] = dsk_convert_track(dsk, drive, data, t, type);
      }
   } else if (type == EWM_DSK_TYPE_NIB) {
      for (int t = 0; t < EWM_DSK_TRACKS; t++) {
         drive->tracks[t].length = 6656;
         drive->tracks[t].data = malloc(6656);
         memcpy(drive->tracks[t].data, data + (t * 6656), 6656);
      }
   }

   return 0;
}

static int ewm_dsk_type_from_path(char *path) {
   if (ewm_utl_endswith(path, ".dsk") || ewm_utl_endswith(path, ".do")) {
      return EWM_DSK_TYPE_DO;
   }
   if (ewm_utl_endswith(path, ".po")) {
      return EWM_DSK_TYPE_PO;
   }
   if (ewm_utl_endswith(path, ".nib")) {
      return EWM_DSK_TYPE_NIB;
   }
   return EWM_DSK_TYPE_UNKNOWN;
}

int ewm_dsk_set_disk_file(struct ewm_dsk_t *dsk, uint8_t drive, bool readonly, char *path) {
   int type = ewm_dsk_type_from_path(path);
   if (type == EWM_DSK_TYPE_UNKNOWN) {
      return -1;
   }

   int fd = open(path, O_RDONLY);
   if (fd == -1) {
      return -1;
   }

   struct stat file_info;
   if (fstat(fd, &file_info) == -1) {
      close(fd);
      return -1;
   }

   if (type == EWM_DSK_TYPE_DO || type == EWM_DSK_TYPE_PO) {
      if (file_info.st_size != (EWM_DSK_TRACKS * EWM_DSK_SECTORS * 256)) {
         close(fd);
         return -1;
      }
   } else if (type == EWM_DSK_TYPE_NIB) {
      if (file_info.st_size != (EWM_DSK_TRACKS * EWM_DSK_NIBBLES_PER_TRACK)) {
         close(fd);
         return -1;
      }
   }

   char *data = calloc(file_info.st_size, 1);
   if (read(fd, data, file_info.st_size) != file_info.st_size) {
      close(fd);
      return -1;
   }

   close(fd);

   int result = ewm_dsk_set_disk_data(dsk, drive, readonly, data, file_info.st_size, type);
   free(data);

   return result;
}
