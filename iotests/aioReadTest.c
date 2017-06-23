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

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDr:t:k:o:q:f:s:G:")) != -1) {
    switch (opt) {
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    case 'q':
      qd = atoi(optarg);
      break;
    case 's':
      seqFiles = atoi(optarg);
      break;
    case 'G':
      maxSizeGB = atof(optarg);
      break;
    case 'k':
      BLKSIZE = 1024 * atoi(optarg);
      break;
    case 'f':
      path = optarg;
      break;
    default:
      exit(-1);
    }
  }
  if (path == NULL) {
    fprintf(stderr,"./aioReadTest [-s sequentialFiles] [-k blocksizeKB] [-q queueDepth] [-t 30 secs] [-G 32] -f blockdevice\n");
    exit(1);
  }
}



int main(int argc, char *argv[]) {
  handle_args(argc, argv);

  size_t seed = (size_t) timedouble();
  srand(seed);
  fprintf(stderr,"path: %s, max queue depth: %d, seed %zd, blocksize: %d", path, qd, seed, BLKSIZE);
  int fd = open(path, O_RDONLY | O_DIRECT);
  if (fd < 0) {perror(path);return -1; }

  size_t bdSize = blockDeviceSize(path);
  if (maxSizeGB >0) {
    bdSize = (size_t) (maxSizeGB * 1024L * 1024 * 1024);
  }
  fprintf(stderr,", bdSize %.1lf GB\n", bdSize/1024.0/1024/1024);

  
  const size_t num = 10*1000*1000;
  size_t *positions = malloc(num * sizeof(size_t));
  const size_t gap = bdSize / (seqFiles + 1);

  for (size_t i = 0; i < num; i++) {
    if (seqFiles == 0) {
      positions[i] = ((size_t)((rand() % bdSize) / BLKSIZE)) * BLKSIZE;
    } else {
      positions[i] = ((i % seqFiles) * gap) + ((i / seqFiles) * BLKSIZE);
    }
    if (i < 10) {
      fprintf(stderr,"[%zd/%zd] pos %zd (%.1lf%%)\n", i+1, num, positions[i], positions[i]*100.0/bdSize);
    }
  }
  
  readMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qd);
  
  return 0;
}
