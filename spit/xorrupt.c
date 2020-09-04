#define _XOPEN_SOURCE
#define _POSIX_C_SOURCE 200809L
#define _ISOC11_SOURCE
#define _DEFAULT_SOURCE

#include "jobType.h"
#include <signal.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <sys/ioctl.h>
#include <malloc.h>
#include <stdio.h>
#include <assert.h>

/**
 * xorrupt.c
 *
 * a byte at a time device XOR corruptor
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

void intHandler(int d)
{
  if (d) {}
  keepRunning = 0;
}

unsigned char *oldBytes = NULL;
int fd;

size_t maxPositions = 0;
size_t *positions = NULL;

void addToPositions(size_t pos) {
  fprintf(stderr,"*info* device position[#%zd]: %zu\n", maxPositions+1, pos);
  positions = realloc(positions, (maxPositions+1) *sizeof(size_t));
  positions[maxPositions] = pos;

  oldBytes = realloc(oldBytes, (maxPositions+1) * sizeof(unsigned char));
  oldBytes[maxPositions] = 0;
  maxPositions++;
}

void freePositions() {
  if (positions) free(positions);
  positions = NULL;
  if (oldBytes) free(oldBytes);
  oldBytes = NULL;
}
  

void perturbBytes() {
  for (size_t i = 0; i <maxPositions; i++) {
    size_t pos = positions[i];
    
    int r = pread(fd, oldBytes + i, 1, pos);
    if (r <= 0) {
      fprintf(stderr,"*error* offset[%zd] did not return a value\n", pos);
      exit(1);
    }
    assert(r == 1);
    fprintf(stderr,"storing %zd: ASCII %d ('%c')... ", pos, oldBytes[i], oldBytes[i]);
    unsigned char newv = 255 ^ oldBytes[i];
    r = pwrite(fd, (void*)&newv, 1, pos);
    assert(r == 1);
    fprintf(stderr,"overwriting %zd: ASCII %d ('%c')\n", pos, newv, newv);
  }
}


void restoreBytes() {
  for (size_t i = 0; i < maxPositions; i++) {
    size_t pos = positions[i];
    
    fprintf(stderr,"restoring %zd: ASCII %d ('%c')... ", pos, oldBytes[i], oldBytes[i]);
    int r = pwrite(fd, oldBytes + i, 1, pos);
    assert(1==r);
    
    unsigned char check;
    r = pread(fd, (void*)&check, 1, pos);
    assert(check == oldBytes[i]);
    if (r==1) {
      fprintf(stderr,"verified the restoration\n");
    }
  }
}



int main(int argc, char *argv[])
{
  int opt;

  char *device = NULL;
  size_t pos = 0;
  size_t runtime = 60;

  optind = 0;
  while ((opt = getopt(argc, argv, "f:p:")) != -1) {
    switch (opt) {
    case 'p':
      if (strchr(optarg,'G') || strchr(optarg,'g')) {
	pos = (size_t)(1024.0 * 1024.0 * 1024.0 * atof(optarg));
      } else if (strchr(optarg,'M') || strchr(optarg,'m')) {
	pos = (size_t)(1024.0 * 1024.0 * atof(optarg));
      } else if (strchr(optarg,'K') || strchr(optarg,'k')) {
	pos = (size_t)(1024 * atof(optarg));
      } else {
	pos = (size_t)atol(optarg);
      }
      addToPositions(pos);
      break;
    case 'f':
      device = strdup(optarg);
      break;
    case 't':
      runtime = atoi(optarg);
      break;
    default:
      exit(1);
      break;
    }
  }

  // first assign the device
  // if one -f specified, add to all jobs
  // if -F specified (with n drives), have c x n jobs
  if (!device) {
    fprintf(stderr,"*info* xorrupt -f device [-p position ... -p position] [-t time]\n");
    fprintf(stderr,"\nTemporarily XOR a byte at an offset position, sleep then restore\n");
    fprintf(stderr,"\nOptions:\n");
    fprintf(stderr,"   -f device  specifies a block device\n");
    fprintf(stderr,"   -p offset  the block device offset [defaults to byte 0]\n"); 
    fprintf(stderr,"   -p 2k      can use k,M,G units\n");
    fprintf(stderr,"   -p 4M      can use k,M,G units\n");
    fprintf(stderr,"   -p 10G     can use k,M,G units\n");
    fprintf(stderr,"   -t time    the time until restore [defaults to %zd]\n", runtime);
    fprintf(stderr,"\n");
    fprintf(stderr,"Note:\n");
    fprintf(stderr,"  Control-C can be used to cancel the timer. The byte is still restored\n");
    exit(1);
  }

  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  fd = open(device, O_RDWR| O_SYNC, S_IRUSR | S_IWUSR);
  if (fd <= 0) {
    perror(device);
  } else {
    perturbBytes();
    
    fprintf(stderr,"*info* pausing for %zd seconds (or control-c), then restore\n", runtime);
    double start = timedouble();
    while (keepRunning && (timedouble() < start + runtime)) {
      sleep(1);
    }
    
    restoreBytes();
  }


  if (device) free(device);

  freePositions();
  
  fflush(stderr);

  exit(0);
}


