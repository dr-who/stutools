#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>
#include <assert.h>

#include "aioRequests.h"
#include "utils.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted
int    exitAfterSeconds = 2;
int    qd = 32;
char   *path = NULL;
int    seqFiles = 0;
double maxSizeGB = 0;
int    BLKSIZE=65536;
size_t jumpStep = 1;
double readRatio = 1.0;
size_t table = 0;

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDr:t:k:o:q:f:s:G:j:p:T")) != -1) {
    switch (opt) {
    case 'T':
      table = 1;
      break;
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    case 'q':
      qd = atoi(optarg); if (qd < 1) qd = 1;
      break;
    case 's':
      seqFiles = atoi(optarg); if (seqFiles < 0) seqFiles = 0;
      break;
    case 'j':
      jumpStep = atoi(optarg); if (jumpStep < 1) jumpStep = 1;
      break;
    case 'G':
      maxSizeGB = atof(optarg); 
      break;
    case 'k':
      BLKSIZE = 1024 * atoi(optarg); if (BLKSIZE < 1024) BLKSIZE=1024;
      break;
    case 'p':
      readRatio = atof(optarg);
      if (readRatio < 0) readRatio = 0;
      if (readRatio > 1) readRatio = 1;
      break;
    case 'f':
      path = optarg;
      break;
    default:
      exit(-1);
    }
  }
  if (path == NULL) {
    fprintf(stderr,"./aioRWTest [-s sequentialFiles] [-j jumpBlocks] [-k blocksizeKB] [-q queueDepth] [-t 30 secs] [-G 32] [-p readRatio] -f blockdevice\n");
    fprintf(stderr,"\nExample:\n");
    fprintf(stderr,"  ./aioRWTest -p0.75 -f /dev/nbd0 -k4 -q64 -s32 -j16 -G100 # 75%% reads\n");
    fprintf(stderr,"  ./aioRWTest -p0.0 -f /dev/nbd0 -k4 -q64 -s32 -j16 -G100  # 0%% reads, 100%% writes\n");
    exit(1);
  }
}



int main(int argc, char *argv[]) {
  handle_args(argc, argv);

  size_t seed = (size_t) timedouble();
  srand48(seed);
  int fd = open(path, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
  if (fd < 0) {perror(path);return -1; }

  size_t origbdSize = blockDeviceSize(path);
  size_t bdSize = origbdSize;
  if (maxSizeGB > 0) {
    bdSize = (size_t) (maxSizeGB * 1024L * 1024 * 1024);
  }
  if (bdSize > origbdSize) {
    bdSize = origbdSize;
  }

  
  const size_t num = 10*1000*1000;
  size_t *positions = malloc(num * sizeof(size_t));
  size_t *randpositions = malloc(num * sizeof(size_t));
  size_t *ppp = NULL;
  size_t gap = 0;

  if (seqFiles > 0) {
    gap = bdSize / (seqFiles);
    gap = (gap >> 16) <<16;
    ppp = calloc(seqFiles, sizeof(size_t));
    for (size_t i = 0; i < seqFiles; i++) {
      ppp[i] = i * gap;
    }
  }

  for (size_t i = 0; i < num; i++) {
    randpositions[i] = (lrand48() % (bdSize / BLKSIZE)) * BLKSIZE;
  }

  if (seqFiles > 0) {
    for (size_t i = 0; i < num; i++) {
      // sequential
      positions[i] = ppp[i % seqFiles];
      ppp[i % seqFiles] += (jumpStep * BLKSIZE);
      
      assert((positions[i]>>16) << 16 == positions[i]);
    }
  }

  if (table) {
    size_t bsArray[]={BLKSIZE};
    size_t qdArray[]={1, 8, 32};
    double rrArray[]={1.0, 0, 0.5};
    size_t ssArray[]={0, 1, 8, 32};

    fprintf(stderr,"blockSz\tnumSeq\tQueueD\tR/W\tIOPS\tMB/s\n");
    for (size_t rrindex=0; rrindex<3; rrindex++) {
      for (size_t ssindex=0; ssindex <4; ssindex++) {
	for (size_t qdindex=0; qdindex<3; qdindex++) {
	  for (size_t bsindex=0; bsindex<1; bsindex++) {
	    double ios = 0;
	    
	    fprintf(stderr,"%zd\t%zd\t%zd\t%4.2f\t", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    
	    double start = timedouble();
	    if (ssArray[ssindex] == 0) {
	      ios = readMultiplePositions(fd, randpositions, num, BLKSIZE, exitAfterSeconds, qd, readRatio, 0);
	    } else {
	      ios = readMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qd, readRatio, 0);
	    }
	    double elapsed = timedouble() - start;
	    
	    fprintf(stderr,"%6.0lf\t%6.0lf\n", ios/elapsed, ios*BLKSIZE/elapsed/1024.0/1024.0);
	    fsync(fd);
	    fdatasync(fd);
	  }
	}
      }
    }
  } else {
    fprintf(stderr,"path: %s, readRatio: %.2lf, max queue depth: %d, seed %zd, blocksize: %d", path, readRatio, qd, seed, BLKSIZE);
    fprintf(stderr,", bdSize %.1lf GB\n", bdSize/1024.0/1024/1024);
    readMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qd, readRatio, 1);
  }
  
  free(positions);
  free(ppp);
  
  return 0;
}
