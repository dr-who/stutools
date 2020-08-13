#define _POSIX_C_SOURCE 200809L


#include "jobType.h"
#include <fcntl.h>
#include <stdlib.h>
#include <stdio.h>
#include <malloc.h>
#include <assert.h>

/**
 * devcontents 
 *
 * iterate through a device, displaying the contents
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"
#include "diskStats.h"

#define DEFAULTTIME 10

int verbose = 0;
int keepRunning = 1;


void analyse(unsigned char *buf, int low, int high, int *m, int *x, int *range) {
  int min = 256;
  int max = 0;
  
  for (int i = low; i < high; i++) {
    if (buf[i] > max) max=buf[i];
    if (buf[i] < min) min=buf[i];
  }
  *m = min;
  *x = max;
  *range = max - min + 1;
}

      
int main(int argc, char *argv[])
{
  int opt;

  char *device = NULL;
  size_t blocksize = 4*1024, width = 50;
  size_t startAt = 0*1024*1024, finishAt = 1024L*1024L*1024L*1;

  optind = 0;
  while ((opt = getopt(argc, argv, "G:g:w:b:f:")) != -1) {
    switch (opt) {
    case 'b':
      blocksize = alignedNumber(atoi(optarg), 4096);
      break;
    case 'f':
      device = optarg;
      break;
    case 'G':
      if (strchr(optarg,'M') || strchr(optarg,'m')) {
	finishAt = (size_t)(1024.0 * 1024.0 * atof(optarg));
      } else {
	finishAt = (size_t)(1024.0 * 1024.0 * 1024.0 * atof(optarg));
      }
      //      fprintf(stderr,"*info* finish at %zd (%.4lf GiB, %.3lf MiB)\n", finishAt, TOGiB(finishAt), TOMiB(finishAt));
      break;
    case 'g':
      if (strchr(optarg,'M') || strchr(optarg,'m')) {
	startAt = (size_t)(1024.0 * 1024.0 * atof(optarg));
      } else {
	startAt = (size_t)(1024.0 * 1024.0 * 1024.0 * atof(optarg));
      }
      //      fprintf(stderr,"*info* start at %zd (%.4lf GiB, %.3lf MiB)\n", startAt, TOGiB(startAt), TOMiB(startAt));
      break;
    case 'w':
      width = atoi(optarg);
      fprintf(stderr,"*info* display contents width = %zd\n", width);
      break;
    default:
      exit(1);
      break;
    }
  }

  // align the numbers to the blocksize
  startAt = alignedNumber(startAt, blocksize);
  finishAt = alignedNumber(finishAt, blocksize);
  
  if (!device) {
    fprintf(stderr,"*info* devcontents -f /dev/device [options...)\n");
    fprintf(stderr,"\nOptions:\n");
    fprintf(stderr,"   -f dev    specify the device\n");
    fprintf(stderr,"   -g n      starting at n GiB (defaults byte 0)\n");
    fprintf(stderr,"   -g 16M    starting at 16 MiB\n");
    fprintf(stderr,"   -G n      finishing at n GiB (defaults to 1 GiB)\n");
    fprintf(stderr,"   -G 32M    finishing at 32 MiB\n");
    fprintf(stderr,"   -b n      the block size to step through the devices (defaults to 256 KiB)\n");
    fprintf(stderr,"   -w n      first n bytes per 4096 block to display (defaults to %zd)\n", width);
    exit(1);
  }

  fprintf(stderr,"*info* aligned start:  %lx (%zd, %.3lf MiB, %.4lf GiB)\n", startAt, startAt, TOMiB(startAt), TOGiB(startAt));
  fprintf(stderr,"*info* aligned finish: %lx (%zd, %.3lf MiB, %.4lf GiB)\n", finishAt, finishAt, TOMiB(finishAt), TOGiB(finishAt));

  int fd = open(device, O_RDONLY, S_IRUSR | S_IWUSR);
  if (fd < 0) {
    perror(device);
  } else {
    unsigned char buf[4096];
    int firstgap = 0;

    fprintf(stdout,"%9s\t%6s\t%8s\t%6s\t%6s\t%6s\t%6s\t%s\n", "pos", "p(MiB)", "pos", "p%256K", "min", "max", "range", "contents...");
    
    unsigned char *pbuf = NULL;
    pbuf = malloc(width + 1);
    if (!pbuf) {
      fprintf(stderr,"*error* OOM\n");
      exit(1);
    }
    for (size_t pos = startAt; pos < finishAt; pos += blocksize) {
      int min = 0, max = 0, range = 0;
      int r = pread(fd, buf, 4096, pos);
      if (r == 0) {
	perror(device);
	exit(1);
      }
      analyse(buf, 0, 4096, &min, &max, &range);
      if (range > 1) {
	memcpy(pbuf, buf, width);
	pbuf[width] = 0;
	for (size_t j = 0; j < width; j++) {
	  if (pbuf[j] < 32) pbuf[j] = '_';
	  if (pbuf[j] >= 127) pbuf[j] = ' ';
	}
	fprintf(stdout,"0x%07zx\t%6.1lf\t%8zd\t%6zd\t%6d\t%6d\t%6d\t%s\n", pos, TOMiB(pos), pos, pos % (256*1024), min, max, range, pbuf);
	firstgap = 1;
      } else {
	if (firstgap) {
	  fprintf(stdout,"...\n");
	  firstgap = 0;
	}
      }
    }
    if (pbuf) {
      free(pbuf);
    }
      
    close(fd);
  }
  

  fprintf(stderr,"*info* exiting.\n");
  fflush(stderr);

  exit(0);
}


