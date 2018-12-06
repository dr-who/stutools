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
#include "devices.h"
#include "utils.h"
#include "blockVerify.h"
  
int verbose = 1;
int keepRunning = 1;
size_t waitEvery = 0;
size_t flushEvery = 0;


void handle_args(int argc, char *argv[], jobType *j, deviceDetails **deviceList, size_t *deviceCount, size_t *maxSizeInBytes, size_t *lowbs, size_t *timetorun) {
  int opt;

  *lowbs = 4096;
  
  jobInit(j);
  
  while ((opt = getopt(argc, argv, "c:f:G:k:t:")) != -1) {
    switch (opt) {
    case 'c':
      jobAdd(j, optarg);
      break;
    case 'f':
      addDeviceDetails(optarg,  deviceList, deviceCount);
      break;
    case 'G':
      *maxSizeInBytes = (*lowbs) * (size_t)(atof(optarg) * 1024 * 1024 * 1024 / (*lowbs));
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
  //  jobDump(j);

  if (*maxSizeInBytes == 0) {
    *maxSizeInBytes = totalRAM() * 2;
  }

  for (size_t i = 0; i < *deviceCount; i++) {
    (*deviceList)[i].shouldBeSize = *maxSizeInBytes;
  }

  openDevices(*deviceList, *deviceCount, 0, maxSizeInBytes, *lowbs, *lowbs, *lowbs, 1, 0, 256, 1);
  int anyopen = 0;
  for (size_t j = 0; j < *deviceCount; j++) {
    if ((*deviceList[j]).fd > 0) {
      anyopen = 1;
    }
  }
  if (!anyopen) {
    fprintf(stderr,"*error* there are no valid block devices\n");
    exit(-1);
  }

  infoDevices(*deviceList, *deviceCount);


}

void usage() {
  fprintf(stderr,"\nExamples: \n");
  fprintf(stderr,"  spit -f device -c r\n");
  fprintf(stderr,"  spit -f device -c w\n");
  fprintf(stderr,"  spit -f device -c rs0    # random\n");
  fprintf(stderr,"  spit -f device -c ws128  # 128 parallel writes\n");
  fprintf(stderr,"  spit -f device -c \"r s128 k4\" -c \'w s4 -k128\' -c rw\n");
  fprintf(stderr,"  spit -f device -c r -G 1 # 1 GiB device size\n");
  fprintf(stderr,"  spit -t 50               # run for 50 seconds\n");
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
  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0, maxSizeInBytes = 0, timetorun = 10;

  size_t lowbs = 4096;
  
  fprintf(stderr,"*info* spit %s %s \n", argv[0], VERSION);
  handle_args(argc, argv, j, &deviceList, &deviceCount, &maxSizeInBytes, &lowbs, &timetorun);
  if (j->count == 0 || deviceCount == 0) {
    usage();
  }
  fprintf(stderr,"*info* maxSizeInBytes %zd\n", maxSizeInBytes);

  jobRunThreads(j, j->count, maxSizeInBytes, lowbs, &(deviceList[0]), timetorun);

  jobFree(j);
  free(j);
  freeDeviceDetails(deviceList, deviceCount);


  exit(0);
}
  
  
