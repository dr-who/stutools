#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include "utils.h"
#include "logSpeed.h"

int keepRunning = 1;


int main(int argc, char *argv[]) {

  int opt, verbose = 0, header = 1;
  
  while ((opt = getopt(argc, argv, "hV" )) != -1) {
    switch (opt) {
    case 'h':
      header = 1;
      break;
    case 'V':
      verbose++;
      break;
    }
  }

  if (optind >= argc) {
    fprintf(stderr, "*info* usage ./trim [-V] device ... device\n");
    exit(1);
  }

  char *dev = NULL;
  for (;optind < argc; optind++) {
    dev = argv[optind];

    unsigned long low = 0;
    unsigned long high = fileSizeFromName(dev);
    double maxdelay_secs = 0;

    if (header) {
      //              "/dev/sda 8.332   ntf(stdout, "device\ttime (s)\tmax (s)\tsize\tTRIM size\n");
      fprintf(stdout, "device   \ttime(s)\tmax(s)\tsize\tTRIM size\n");
      header = 0;
    }
    if (isBlockDevice(dev) == 1) {
      int fd = open(dev, O_RDWR | O_EXCL | O_DIRECT );
      if (fd >= 0) {
	size_t d_max_bytes = 0, d_granularity = 0, d_zeroes = 0;
	getDiscardInfo(getSuffix(dev), &d_max_bytes, &d_granularity, &d_zeroes);

	if (verbose >= 2) {
	  fprintf(stderr,"*info* max_bytes: %zd\n", d_max_bytes);
	  fprintf(stderr,"*info* granularity: %zd\n", d_granularity);
	  fprintf(stderr,"*info* zeroes data: %zd\n", d_zeroes);
	}
      
	if (fd >= 0) {
	  double start = timedouble();
	  performDiscard(fd, dev, low, high, d_max_bytes, d_granularity, &maxdelay_secs, verbose);
	  close(fd);
	  double elapsed = timedouble() - start;
	  fprintf(stdout, "%s\t%.3lf\t%.3lf\t%.0lf GB\t%zd\n", dev, elapsed, maxdelay_secs, TOGB(high), d_max_bytes);
	} else {
	  perror(dev);
	}
      } else {
	fprintf(stderr,"*error* couldn't open %s exclusively\n", dev);
      }
    } else {
      fprintf(stderr,"*error* a block device is required as an argument\n");
    }
  }

  return 0;
}
