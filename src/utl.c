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

#include <stddef.h>
#include <string.h>

#if __APPLE__ && __MACH__
#include <sys/_types/_timespec.h>
#include <mach/mach.h>
#include <mach/clock.h>
#endif

#include "utl.h"

bool ewm_utl_endswith(char *s, char *suffix) {
   if (s != NULL && suffix != NULL) {
      if (strlen(suffix) <= strlen(s)) {
         return strcmp(s + strlen(s) - strlen(suffix), suffix) == 0;
      }
   }
   return false;
}

#if (defined(__MAC_OS_X_VERSION_MIN_REQUIRED) && __MAC_OS_X_VERSION_MIN_REQUIRED < 101200)
int clock_gettime(clockid_t clk_id, struct timespec *tp) {
   if (clk_id != CLOCK_REALTIME) {
      return -1;
   }

   kern_return_t result = KERN_SUCCESS;

   clock_serv_t cclock;
   mach_timespec_t mts;

   host_get_clock_service(mach_host_self(), clk_id, &cclock);
   result = clock_get_time(cclock, &mts);
   mach_port_deallocate(mach_task_self(), cclock);

   tp->tv_sec = mts.tv_sec;
   tp->tv_nsec = mts.tv_nsec;

   return result;
}
#endif
