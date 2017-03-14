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

#include <getopt.h>

#include <SDL2/SDL.h>

#include "one.h"
#include "two.h"
#include "boo.h"

static void usage() {
   fprintf(stderr, "Usage: ewm [--help|-h] [<command> [--help|-h] [args]]\n");
   fprintf(stderr, "\n");
   fprintf(stderr, "Commands:\n");
   fprintf(stderr, "  one     Run the Apple 1 emulator\n");
   fprintf(stderr, "  two     Run the Apple ][+ emulator\n");
   fprintf(stderr, "  boo     Run the 'bootloader' (default)\n");
   fprintf(stderr, "\n");
   fprintf(stderr, "If no command is specified, the 'bootloader' will be run, which\n");
   fprintf(stderr, "allows the user to interactively select what emulator to start.\n");
}

int main(int argc, char **argv) {
   if (argc == 1) {
      switch (ewm_boo_main(argc, argv)) {
         case EWM_BOO_BOOT_APPLE1: {
            char *args[] = { "one", "-model", "apple1", NULL };
            return ewm_one_main(3, args);
         }
         case EWM_BOO_BOOT_REPLICA1: {
            char *args[] = { "one", "-model", "replica1", NULL };
            return ewm_one_main(3, args);
         }
         case EWM_BOO_BOOT_APPLE2PLUS: {
            char *args[] = { "two", NULL };
            return ewm_two_main(1, args);
         }
      }
   } else if (argc > 1) {
      if (strcmp(argv[1], "--help") == 0 || strcmp(argv[1], "-h") == 0) {
         usage();
         exit(0);
      }

      // Delegate to the Apple 1 / Replica 1 emulation
      if (strcmp(argv[1], "one") == 0) {
         return ewm_one_main(argc-1, &argv[1]);
      }

      // Delegate to the Apple ][+ emulation
      if (strcmp(argv[1], "two") == 0) {
         return ewm_two_main(argc-1, &argv[1]);
      }

      // Delegate to the bootloader
      if (strcmp(argv[1], "boo") == 0) {
         return ewm_boo_main(argc-1, &argv[1]);
      }
   }

   usage();
   return 1;
}
