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
  size_t minSize = 0, maxSize = 0, zeroall = 0;
  
  while ((opt = getopt(argc, argv, "hVG:g:z" )) != -1) {
    switch (opt) {
    case 'h':
      header = 1;
      break;
    case 'G':
      {}
      size_t scale = 1024L * 1024L * 1024L;
      if (strchr(optarg, 'K') || strchr(optarg, 'k')) {
	scale = 1024;
      } else if (strchr(optarg, 'M') || strchr(optarg, 'm')) {
	scale = 1024L * 1024L;
      }
      maxSize = alignedNumber(atof(optarg) * scale, 4096);
      break;
    case 'g':
      {}
      scale = 1024L * 1024L * 1024L;
      if (strchr(optarg, 'K') || strchr(optarg, 'k')) {
	scale = 1024;
      } else if (strchr(optarg, 'M') || strchr(optarg, 'm')) {
	scale = 1024L * 1024L;
      }
      minSize = alignedNumber(atof(optarg) * scale, 4096);
      break;
    case 'z':
      zeroall = 1;
      break;
    case 'V':
      verbose++;
      break;
    }
  }

  if (optind >= argc) {
    fprintf(stderr, "*info* usage ./trim [-V] [-g size] [-G size] device ... device\n");
    fprintf(stderr,"\nExamples:\n");
    fprintf(stderr,"  # actually write zeros, TRIM doesn't always zero\n");
    fprintf(stderr,"  ./trim -G 16M -z /dev/sd*  \n\n");
    fprintf(stderr,"  # trim from 1 GiB to 2 GiB\n");
    fprintf(stderr,"  ./trim -G 16M /dev/sd*  \n\n");
    fprintf(stderr,"  # trim from 1 GiB to 2 GiB\n");
    fprintf(stderr,"  ./trim -g 1G -G 2G /dev/sd*  \n\n");
    fprintf(stderr,"  # trim from 4-16 KiB\n");
    fprintf(stderr,"  ./trim -g 4k -G 16k /dev/sd*  \n");
    exit(1);
  }

  char *dev = NULL;
  for (;optind < argc; optind++) {
    dev = argv[optind];

    unsigned long low = 0;
    unsigned long high = fileSizeFromName(dev);

    if (maxSize > 0) {
      if (maxSize < high) {
	high = maxSize;
      }
    }
    if (minSize > 0) {
      low = minSize;
    }
    if (low > high) low = high;

    double maxdelay_secs = 0;

    if (header) {
      //              "/dev/sda 8.332   ntf(stdout, "device\ttime (s)\tmax (s)\tsize\tTRIM size\n");
      fprintf(stdout, "device   \t(GiB) -\t(GiB)\ttime(s)\tmax(s)\tTRIM size\n");
      header = 0;
    }
    if (isBlockDevice(dev) == 1) {
      int fd = open(dev, O_RDWR | O_EXCL | O_DIRECT );
      if (fd >= 0) {
	size_t d_max_bytes = 0, d_granularity = 0, d_zeroes = 0, alignment = 0;
	getDiscardInfo(getSuffix(dev), &alignment, &d_max_bytes, &d_granularity, &d_zeroes);
	  

	if (verbose >= 2) {
	  fprintf(stderr,"*info* alignment: %zd\n", alignment);
	  fprintf(stderr,"*info* max_bytes: %zd\n", d_max_bytes);
	  fprintf(stderr,"*info* granularity: %zd\n", d_granularity);
	  fprintf(stderr,"*info* zeroes data: %zd\n", d_zeroes);
	}
      
	if (fd >= 0) {
	  double start = timedouble();
	  if (d_zeroes) {
	    fprintf(stderr,"*info* trim zeroes the data\n");
	  }
	  //	  fprintf(stderr,"dev %s, %d\n", dev, (d_zeroes==0) && zeroall);
	  performDiscard(fd, dev, low, high, d_max_bytes, d_granularity, &maxdelay_secs, verbose, (d_zeroes==0) && zeroall);
	  close(fd);
	  double elapsed = timedouble() - start;
	  fprintf(stdout, "%s\t%.3lf\t%.3lf\t%.3lf\t%.3lf\t%zd\n", dev, TOGiB(low), TOGiB(high), elapsed, maxdelay_secs, d_max_bytes);
	} else {
	  perror(dev);
	}
      } else {
	fprintf(stderr,"*warning* couldn't open %s exclusively... skipping...\n", dev);
      }
    } else {
      fprintf(stderr,"*error* a block device is required as an argument\n");
    }
  }

  return 0;
}
