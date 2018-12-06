#define _POSIX_C_SOURCE 200809L

#include "jobType.h"

/**
 * spit.c
 *
 * the main() for ./spit, Stu's parallel I/O tester
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"
#include "blockVerify.h"
  
int verbose = 1;
int keepRunning = 1;
size_t waitEvery = 0;
size_t flushEvery = 0;


void handle_args(int argc, char *argv[], jobType *j, size_t *maxSizeInBytes, size_t *lowbs, size_t *timetorun) {
  int opt;

  *lowbs = 4096;
  char *device = NULL;
  int extraparalleljobs = 0, isAFile = 0;
  
  jobInit(j);
  
  while ((opt = getopt(argc, argv, "c:f:G:k:t:j:")) != -1) {
    switch (opt) {
    case 'c':
      jobAdd(j, device, optarg);
      break;
    case 'f':
      device = optarg;
      if (!fileExists(device)) { // nothing is there, create a file
	fprintf(stderr,"*warning* will need to create '%s'\n", device);
	isAFile = 1;
      } else {
	// it's there
	if (isBlockDevice(device) == 2) {
	  // it's a file
	  isAFile = 1;
	} 
	
	if (!canOpenExclusively(device)) {
	  fprintf(stderr,"*error* can't open '%s' exclusively\n", device);
	  exit(-1);
	}
      }
      break;
    case 'G':
      *maxSizeInBytes = (*lowbs) * (size_t)(atof(optarg) * 1024 * 1024 * 1024 / (*lowbs));
      break;
    case 'j':
      extraparalleljobs = atoi(optarg) - 1;
      if (extraparalleljobs < 0) extraparalleljobs = 0;
      break;
    case 'k':
      *lowbs = 1024 * atoi(optarg);
      if (*lowbs < 512) *lowbs = 512;
      break;
    case 't':
      *timetorun = atoi(optarg);
      break;
    default:
      fprintf(stderr,"nope\n");
      break;
    }
  }

  if (extraparalleljobs) {
    jobMultiply(j, extraparalleljobs);
  }

  size_t fsize = fileSizeFromName(device);
  if (isAFile) {
    if (*maxSizeInBytes == 0) {
      *maxSizeInBytes = totalRAM() * 2;
    }
    if (fsize != *maxSizeInBytes) {
      createFile(device, *maxSizeInBytes);
    }
  } else {
    if (*maxSizeInBytes > fsize || *maxSizeInBytes == 0) {
      *maxSizeInBytes = fsize;
    }
  }

  fprintf(stderr,"*info* maxSizeInBytes %zd (%.3g GiB)\n", *maxSizeInBytes, TOGiB(*maxSizeInBytes));

}

void usage() {
  fprintf(stderr,"\nExamples: \n");
  fprintf(stderr,"  spit -f device -c ... -c ... -c ... # defaults to 10 seconds\n");
  fprintf(stderr,"  spit -f device -c r           # seq 1 read\n");
  fprintf(stderr,"  spit -f device -c w           # seq 1 write\n");
  fprintf(stderr,"  spit -f device -c rs0         # random\n");
  fprintf(stderr,"  spit -f device -c ws128       # 128 parallel writes\n");
  fprintf(stderr,"  spit -f device -c rs128P1000  # 128 parallel writes, 1000 positions\n");
  fprintf(stderr,"  spit -f device -c \"r s128 k4\" -c \'w s4 -k128\' -c rw\n");
  fprintf(stderr,"  spit -f device -c r -G 1      # 1 GiB device size\n");
  fprintf(stderr,"  spit -t 50                    # run for 50 seconds\n");
  exit(-1);
}



/**
 * main
 *
 */
int main(int argc, char *argv[]) {
#ifndef VERSION
#define VERSION __TIMESTAMP__
#endif

  jobType *j = malloc(sizeof(jobType));
  size_t maxSizeInBytes = 0, timetorun = 10, lowbs = 4096;
  
  fprintf(stderr,"*info* spit %s %s \n", argv[0], VERSION);
  handle_args(argc, argv, j, &maxSizeInBytes, &lowbs, &timetorun);
  if (j->count == 0) {
    usage();
  }

  jobRunThreads(j, j->count, maxSizeInBytes, lowbs, timetorun);

  jobFree(j);
  free(j);

  exit(0);
}
  
  
