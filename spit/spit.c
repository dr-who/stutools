#define _POSIX_C_SOURCE 200809L

#include "jobType.h"
#include <signal.h>

/**
 * spit.c
 *
 * the main() for ./spit, Stu's powerful I/O tester
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"

#define DEFAULTTIME 10
  
int verbose = 0;
int keepRunning = 1;
char *benchmarkName = NULL;

int handle_args(int argc, char *argv[], jobType *j, size_t *maxSizeInBytes, size_t *timetorun,
		 size_t *dumpPositions) {
  int opt;

  char *device = NULL;
  int extraparalleljobs = 0;

  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0;

  jobInit(j);
  
  while ((opt = getopt(argc, argv, "c:f:G:t:j:d:VB:I:")) != -1) {
    switch (opt) {
    case 'B':
      benchmarkName = strdup(optarg);
      break;
    case 'c':
      jobAdd(j, optarg);
      break;
    case 'd':
      *dumpPositions = atoi(optarg);
      break;
    case 'I':
      {}
      size_t added = loadDeviceDetails(optarg, &deviceList, &deviceCount);
      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
      break;
    case 'f':
      device = optarg;
      break;
    case 'G':
      *maxSizeInBytes = 1024 * (size_t)(atof(optarg) * 1024 * 1024);
      break;
    case 'V':
      verbose++;
      break;
    case 'j':
      extraparalleljobs = atoi(optarg) - 1;
      if (extraparalleljobs < 0) extraparalleljobs = 0;
      break;
    case 't':
      *timetorun = atoi(optarg);
      if (*timetorun == 0) {
	fprintf(stderr,"*error* zero isn't a valid time. -t -1 for a long time\n");
	exit(1);
	//	*timetorun = (size_t)-1; // run for ever
      }
      break;
    default:
      exit(1);
      break;
    }
  }

  // first assign the device
  // if one -f specified, add to all jobs
  // if -F specified (with n drives), have c x n jobs
  if (deviceCount) {
    // if we have a list of files, overwrite the first one, ignoring -f
    if (device) {
      fprintf(stderr,"*warning* ignoring the value from -f\n");
    }
    device = deviceList[0].devicename;
  }

  if (device) {
    // set the first device to all of the jobs
    jobAddDeviceToAll(j, device);
  }

  if (deviceCount) {
    // scale up based on the -I list
    jobMultiply(j, 1, deviceList, deviceCount);
  }

  // scale up using the -j 
  if (extraparalleljobs) {
    jobMultiply(j, extraparalleljobs, NULL, 0);
  }
    
  if (verbose) jobDump(j);
  
  // check the file, create or resize
  size_t fsize = 0;
  for (size_t i = 0; i < jobCount(j); i++) {
    device = j->devices[i];
    size_t isAFile = 0;
    
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
    
    fsize = fileSizeFromName(device);
    if (isAFile) {
      if (*maxSizeInBytes == 0) { // if not specified use 2 x RAM
	*maxSizeInBytes = totalRAM() * 2;
      }
      if (fsize != *maxSizeInBytes) { // check the on disk size
	createFile(device, *maxSizeInBytes);
      }
    } else {
      // if you specify -G too big or it's 0 then set it to the existing file size
      if (*maxSizeInBytes > fsize || *maxSizeInBytes == 0) {
	*maxSizeInBytes = fsize;
      }
    }
  }
  
  return 0;
}

void usage() {
  fprintf(stderr,"\nUsage:\n  spit [-f device] [-c string] [-c string] ... [-c string]\n");
  fprintf(stderr,"\nExamples:\n");
  fprintf(stderr,"  spit -f device -c ... -c ... -c ... # defaults to %d seconds\n", DEFAULTTIME);
  fprintf(stderr,"  spit -f device -c r           # seq read (s1)\n");
  fprintf(stderr,"  spit -f device -c w           # seq write (s1)\n");
  fprintf(stderr,"  spit -f device -c rs0         # random, (s)equential is 0\n");
  fprintf(stderr,"  spit -f device -c ws128       # 128 parallel (s)equential writes\n");
  fprintf(stderr,"  spit -f device -c rs128P1000  # 128 parallel writes, 1000 positions\n");
  fprintf(stderr,"  spit -f device -c k8          # set block size to 8 KiB\n");
  fprintf(stderr,"  spit -f device -c k4-128      # set block range to 4 to 128 KiB\n");
  fprintf(stderr,"  spit -f device -c W5          # wait for 5 seconds before commencing I/O\n");
  fprintf(stderr,"  spit -f device -c \"r s128 k4\" -c \'w s4 -k128\' -c rw\n");
  fprintf(stderr,"  spit -f device -c r -G 1      # 1 GiB device size\n");
  fprintf(stderr,"  spit -f ... -t 50             # run for 50 seconds (-t 0 is forever)\n");
  fprintf(stderr,"  spit -f ... -j 32             # duplicate all the commands 32 times\n");
  fprintf(stderr,"  spit -f ... -f ...-d 10       # dump the first 10 positions per command\n");
  fprintf(stderr,"  spit -f ... -c rD0            # 'D' turns off O_DIRECT\n");
  fprintf(stderr,"  spit -f ... -c w -cW4rs0      # one thread seq write, one thread wait 4 then random read\n");
  fprintf(stderr,"  spit -f ... -c wR42           # set the per command seed with R\n");
  fprintf(stderr,"  spit -f ... -c wF             # (F)lush after every write of FF for 10, FFF for 100 ...\n");
  fprintf(stderr,"  spit -f ... -c rrrrw          # do 4 reads for every write\n");
  fprintf(stderr,"  spit -f ... -c rw             # mix 50/50 reads/writes\n");
  fprintf(stderr,"  spit -f ... -c rn -t0         # generate (n)on-unique positions positions with collisions\n");
  fprintf(stderr,"  spit -f ... -t 0              # -t 0 is run forever\n");
  fprintf(stderr,"  spit -f ... -c wz             # sequentially (w)rite from block (z)ero (instead of random position)\n");
  fprintf(stderr,"  spit -f ... -c m              # non-unique positions, read/write/flush like (m)eta-data\n");
  fprintf(stderr,"  spit -f ... -c mP4000         # non-unique 4000 positions, read/write/flush like (m)eta-data\n");
  fprintf(stderr,"  spit -f ... -c n              # 100,000 (n)on-unique positions, read/write, reseeding every 100,000\n");
  fprintf(stderr,"  spit -f ... -c rL4            # (L)imit positions so the sum of the length is 4 GiB\n");
  fprintf(stderr,"  spit -f ... -c P10x100        # multiply the number of positions by x, here it's 100\n");
  fprintf(stderr,"  spit -f ... -c wM1            # set block size 1M\n");
  fprintf(stderr,"  spit -f ... -c O              # One-shot, not time based\n");
  fprintf(stderr,"  spit -f ... -c t2             # specify the time per thread\n");
  fprintf(stderr,"  spit -f ... -c wx2O           # sequentially (s1) write 200%% LBA, no time limit\n");
  fprintf(stderr,"  spit -I devices.txt -c r      # -I is read devices from a file\n");
  exit(-1);
}


void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
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
  size_t maxSizeInBytes = 0, timetorun = DEFAULTTIME, dumpPositions = 0;

  // don't run if swap is on
  if (swapTotal() > 0) {
    fprintf(stderr,"*error* spit needs swap to be off for believable numbers. `sudo swapoff -a`\n");
  }
  // set OOM adjust to 1,000 to make this program be killed first
  FILE *fp = fopen("/proc/self/oom_score_adj", "wt");
  fprintf(fp,"1000\n");
  fclose(fp);


  
  fprintf(stderr,"*info* spit %s %s (Stu's powerful I/O tester)\n", argv[0], VERSION);
  
  handle_args(argc, argv, j, &maxSizeInBytes, &timetorun, &dumpPositions);
  if (j->count == 0) {
    usage();
  }

  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  fprintf(stderr,"*info* bdSize %.3lf GB (%zd bytes, %.3lf PB)\n", TOGB(maxSizeInBytes), maxSizeInBytes, TOPB(maxSizeInBytes));
  jobRunThreads(j, j->count, maxSizeInBytes, timetorun, dumpPositions, benchmarkName);

  jobFree(j);
  free(j);

  exit(0);
}
  
  
