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
#include "diskStats.h"
#include "fuzz.h"

#define DEFAULTTIME 10
  
int verbose = 0;
int keepRunning = 1;
char *benchmarkName = NULL;

int handle_args(int argc, char *argv[], jobType *preconditions, jobType *j,
		size_t *minSizeInBytes, size_t *maxSizeInBytes, size_t *timetorun, size_t *dumpPositions, size_t *defaultqd,
		unsigned short *seed, diskStatType *d, size_t *verify, double *timeperline, double *ignorefirstsec) {
  int opt;

  char *device = NULL;
  int extraparalleljobs = 0;

  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0;
  size_t tripleX = 0;

  jobInit(j);
  jobInit(preconditions);
  
  while ((opt = getopt(argc, argv, "c:f:G:t:j:d:VB:I:q:XR:p:O:s:i:")) != -1) {
    switch (opt) {
    case 'B':
      benchmarkName = strdup(optarg);
      break;
    case 'c': {}
      size_t jcount = 1;
      char *charJ = strchr(optarg, 'j');
      if (charJ && *(charJ+1)) {
	jcount = atoi(charJ + 1);
	fprintf(stderr,"*info* adding command '%s' x %zd times\n", optarg, jcount);
      }
      for (size_t i = 0; i < jcount; i++) {
	jobAdd(j, optarg);
      }
      break;
    case 'd':
      *dumpPositions = atoi(optarg);
      break;
    case 'i':
      *ignorefirstsec = atof(optarg);
      break;
    case 'I':
      {}
      size_t added = loadDeviceDetails(optarg, &deviceList, &deviceCount);
      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
      break;
    case 'f':
      device = optarg;
      break;
    case 'G': {}
      double lowg = 0, highg = 0;
      splitRange(optarg, &lowg, &highg);
      *minSizeInBytes = alignedNumber(1024L * lowg * 1024 * 1024, 4096);
      *maxSizeInBytes = alignedNumber(1024L * highg * 1024 * 1024, 4096);
      if (*minSizeInBytes == *maxSizeInBytes) { 
	*minSizeInBytes = 0;
      }
      if (*minSizeInBytes > *maxSizeInBytes) {
	fprintf(stderr,"*error* low range needs to be lower [%.1lf, %.1lf]\n", lowg, highg);
	exit(1);
      }
      break;
    case 'j':
      extraparalleljobs = atoi(optarg) - 1;
      if (extraparalleljobs < 0) extraparalleljobs = 0;
      break;
    case 'O':
      diskStatFromFilelist(d, optarg, 0);
      //      fprintf(stderr,"*info* raw devices for amplification analysis: %zd\n", d->numDevices);
      break;
    case 'p': // pre-conditions
      jobAdd(preconditions, optarg);
      break;
    case 'q':
      *defaultqd = atoi(optarg);
      if (*defaultqd < 1) {
	*defaultqd = 1;
      } else if (*defaultqd > 65535) {
	*defaultqd = 65535;
      }
      break;
    case 'R':
      *seed = (unsigned short)atoi(optarg);
      fprintf(stderr,"*info* initial seed: %d\n", *seed);
      break;
    case 's':
      *timeperline = atof(optarg);
      break;
    case 't':
      *timetorun = atoi(optarg);
      if (*timetorun == 0) {
	fprintf(stderr,"*error* zero isn't a valid time. -t -1 for a long time\n");
	exit(1);
	//	*timetorun = (size_t)-1; // run for ever
      }
      break;
    case 'v':
      *verify = 1;
      break;
    case 'V':
      verbose++;
      break;
    case 'X':
      tripleX++;
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
    jobAddDeviceToAll(preconditions, device);
  }

  if (deviceCount) {
    // scale up based on the -I list
    jobMultiply(j, 1, deviceList, deviceCount);
    jobMultiply(preconditions, 1, deviceList, deviceCount);
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

      if (tripleX < 3) {
	if (!canOpenExclusively(device)) {
	  fprintf(stderr,"*error* can't open '%s' exclusively\n", device);
	  exit(-1);
	}
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
	if (*maxSizeInBytes > fsize) {
	  fprintf(stderr,"*warning* limiting size to %zd, ignoring -G\n", *maxSizeInBytes);
	}
	*maxSizeInBytes = fsize;
	if (*minSizeInBytes > fsize) {
	  fprintf(stderr,"*warning* limiting size to %zd, ignoring -G\n", 0L);
	  *minSizeInBytes = 0;
	}
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
  fprintf(stderr,"  spit -f device -c r -G 1-2    # Only perform actions in the 1-2 GiB range\n");
  fprintf(stderr,"  spit -c ws1G1-2 -c rs0G2-3    # Seq w in the 1-2 GiB region, rand r in the 2-3 GiB region\n");
  fprintf(stderr,"  spit -f ... -t 50             # run for 50 seconds (-t -1 is forever)\n");
  fprintf(stderr,"  spit -f ... -j 32             # duplicate all the commands 32 times\n");
  fprintf(stderr,"  spit -f ... -f ...-d 10       # dump the first 10 positions per command\n");
  fprintf(stderr,"  spit -f ... -c rD0            # 'D' turns off O_DIRECT\n");
  fprintf(stderr,"  spit -f ... -c w -cW4rs0      # one thread seq write, one thread wait 4 then random read\n");
  fprintf(stderr,"  spit -f ... -c wR42           # set the per command seed with R\n");
  fprintf(stderr,"  spit -f ... -c wF             # (F)lush after every write of FF for 10, FFF for 100 ...\n");
  fprintf(stderr,"  spit -f ... -c rrrrw          # do 4 reads for every write\n");
  fprintf(stderr,"  spit -f ... -c rw             # mix 50/50 reads/writes\n");
  fprintf(stderr,"  spit -f ... -c n              # shuffles the positions every pass\n");
  fprintf(stderr,"  spit -f ... -c N              # adds an offset to the positions every pass\n");
  fprintf(stderr,"  spit -f ... -t -1             # -t -1 is run forever\n");
  fprintf(stderr,"  spit -f ... -c p0.9           # set the r/w ratio to 0.9\n");
  fprintf(stderr,"  spit -f ... -c wz             # sequentially (w)rite from block (z)ero (instead of random position)\n");
  fprintf(stderr,"  spit -f ... -c m              # non-unique positions, read/write/flush like (m)eta-data\n");
  fprintf(stderr,"  spit -f ... -c mP4000         # non-unique 4000 positions, read/write/flush like (m)eta-data\n");
  fprintf(stderr,"  spit -f ... -c s1n            # do a sequential pass, then shuffles the positions\n");
  fprintf(stderr,"  spit -f ... -c rL4            # (L)imit positions so the sum of the length is 4 GiB\n");
  fprintf(stderr,"  spit -f ... -c P10G1-2        # The first 10 positions starting from 1GiB. It needs the lower range.\n");
  fprintf(stderr,"  spit -f ... -c P10x100        # multiply the number of positions by x, here it's 100\n");
  fprintf(stderr,"  spit -f ... -c wM1            # set block size 1M\n");
  fprintf(stderr,"  spit -f ... -c O              # One-shot, not time based\n");
  fprintf(stderr,"  spit -f ... -c t2             # specify the time per thread\n");
  fprintf(stderr,"  spit -f ... -c ws1J4          # jumble/reverse groups of 4 positions\n");
  fprintf(stderr,"  spit -f ... -c wx2O           # sequentially (s1) write 200%% LBA, no time limit\n");
  fprintf(stderr,"  spit -f ... -c ws0            # random defaults to 3x LBA\n");
  fprintf(stderr,"  spit -f ... -c ws1W2T2 -t60   # Alternate wait 2, run for 2 seconds\n");
  fprintf(stderr,"  spit -I devices.txt -c r      # -I is read devices from a file\n");
  fprintf(stderr,"  spit -f .... -R seed          # set the initial seed, -j will increment per job\n");
  fprintf(stderr,"  spit -f .... -Q qd            # set the per job default queue depth\n");
  fprintf(stderr,"  spit -f .... -c wns0X10       # writing 10 times, not time based\n");
  fprintf(stderr,"  spit -p G -p Gs1              # precondition job, writing random, 100%% LBA, then seq job\n");
  fprintf(stderr,"  spit -c wZ1                   # Z is the starting offset. -z is -Z0\n");
  fprintf(stderr,"  spit -p G100                  # precondition job, writing random overwrite LBA size\n");
  fprintf(stderr,"  spit -p G100s1k64             # precondition job, sequential, 64 KiB blocks\n");
  fprintf(stderr,"  spit -f meta -O devices.txt   # specify the raw devices for amplification statistics\n"); 
  fprintf(stderr,"  spit -s 0.1 -i 5              # and ignore first 5 seconds of performance\n");
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

  if ((argc > 2) && (strcmp(argv[1],"fuzz")==0)) {
    verbose = 2;
    argv = fuzzString(&argc, argv[2]);
  }
  
  jobType *j = malloc(sizeof(jobType));
  jobType *preconditions = malloc(sizeof(jobType));
  
  size_t minSizeInBytes = 0, maxSizeInBytes = 0, timetorun = DEFAULTTIME, dumpPositions = 0;

  // don't run if swap is on
  if (swapTotal() > 0) {
    fprintf(stderr,"*error* spit needs swap to be off for believable numbers. `sudo swapoff -a`\n");
  }
  // set OOM adjust to 1,000 to make this program be killed first
  FILE *fp = fopen("/proc/self/oom_score_adj", "wt");
  fprintf(fp,"1000\n");
  fclose(fp);


  
  fprintf(stderr,"*info* spit %s %s (Stu's powerful I/O tester)\n", argv[0], VERSION);

  size_t defaultQD = 16;
  unsigned short seed = 0;
  diskStatType d;
  size_t verify = 0;
  double timeperline = 1, ignoresec = 0;

  diskStatSetup(&d);
  handle_args(argc, argv, preconditions, j, &minSizeInBytes, &maxSizeInBytes, &timetorun, &dumpPositions, &defaultQD, &seed, &d, &verify, &timeperline, &ignoresec);
  
  if (j->count == 0) {
    usage();
  }


  size_t actualSize = maxSizeInBytes - minSizeInBytes;
  fprintf(stderr,"*info* bdSize range [%.2lf-%.2lf] GB, size %.2lf GB (%zd bytes), [%.3lf-%.3lf] TB\n", TOGB(minSizeInBytes), TOGB(maxSizeInBytes), TOGB(actualSize), actualSize, TOTB(minSizeInBytes), TOTB(maxSizeInBytes));
  if (actualSize < 4096) {
    fprintf(stderr,"*error* block device too small.\n");
    exit(1);
  }

  keepRunning = 1;
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  jobRunPreconditions(preconditions, preconditions->count, minSizeInBytes, maxSizeInBytes);

  keepRunning = 1;
  diskStatType *p = &d;
  if (!d.numDevices) {
    p = NULL;
  }
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  jobRunThreads(j, j->count, minSizeInBytes, maxSizeInBytes, timetorun, dumpPositions, benchmarkName, defaultQD, seed, 1, p, timeperline, ignoresec);

  jobFree(j);
  free(j);

  jobFree(preconditions);
  free(preconditions);
  diskStatFree(&d);

  fprintf(stderr,"*info* exiting.\n"); fflush(stderr);
  exit(0);
}
  
  
