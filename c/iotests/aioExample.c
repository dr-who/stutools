// aioExample.c from stutools
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#include "aioRequests.h"
#include "utils.h"

int keepRunning = 1;
int flushEvery = 0;

int main() {

  int f = open("/dev/sda", O_RDONLY | O_DIRECT);
  if (f < 0) {exit(-1);}

  io_context_t c;
  aioSetup(&c, 32);

  char *data;
  CALLOC(data, 65536, 1); // will be aligned to 4KiB by default

  size_t total = 0, inFlight = 0;

  while (1) {
    struct iocb *io = aioGetContext();
    if (io && total < 1000) {
      int s = aioRead(&c, io, f, (void*) data, 65536, 65536 * inFlight);
      if (s) {
	inFlight++;
	total += s;
      }
    }
    
    int r= aioGet(&c);
    if (r) {
      inFlight -= r;
    }
    free(io);
    fprintf(stderr,"inflight %zd, total %zd\n", inFlight, total);
    if (inFlight == 0) break;
  } // while

  free(data);
  close(f);

  return 0;
}
