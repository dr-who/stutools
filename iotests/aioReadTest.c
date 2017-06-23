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

#include "aioReads.h"
#include "utils.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted
int    exitAfterSeconds = 30;
int    qd = 32;
char   *path = NULL;
int    seqFiles = 0;
double maxSizeGB = 0;
int    BLKSIZE=65536;
size_t jumpStep = 1;
double readRatio = 1.0;

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDr:t:k:o:q:f:s:G:j:p:")) != -1) {
    switch (opt) {
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
  fprintf(stderr,"path: %s, readRatio: %.2lf, max queue depth: %d, seed %zd, blocksize: %d", path, readRatio, qd, seed, BLKSIZE);
  int fd = open(path, O_RDWR | O_DIRECT);
  if (fd < 0) {perror(path);return -1; }

  size_t origbdSize = blockDeviceSize(path);
  size_t bdSize = origbdSize;
  if (maxSizeGB > 0) {
    bdSize = (size_t) (maxSizeGB * 1024L * 1024 * 1024);
  }
  if (bdSize > origbdSize) {
    bdSize = origbdSize;
  }
  fprintf(stderr,", bdSize %.1lf GB\n", bdSize/1024.0/1024/1024);

  
  const size_t num = 10*1000*1000;
  size_t *positions = malloc(num * sizeof(size_t));
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
    if (seqFiles == 0) {
      // random
      positions[i] = (lrand48() % (bdSize / BLKSIZE)) * BLKSIZE;
    } else {
      // sequential
      positions[i] = ppp[i % seqFiles];
      ppp[i % seqFiles] += (jumpStep * BLKSIZE);

      assert((positions[i]>>16) << 16 == positions[i]);
    }
    if (i < seqFiles + 8) {
      fprintf(stderr,"[%zd/%zd] pos %zd (%.1lf%%)\n", i+1, num, positions[i], positions[i]*100.0/bdSize);
    }
  }
  
  readMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qd, readRatio);

  free(positions);
  free(ppp);
  
  return 0;
}
