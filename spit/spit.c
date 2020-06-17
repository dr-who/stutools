#define _POSIX_C_SOURCE 200809L

#include "jobType.h"
#include <signal.h>

#ifndef VERSION
#define VERSION __TIMESTAMP__
#endif

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
#include "spitfuzz.h"

#define DEFAULTTIME 10

int verbose = 0;
int keepRunning = 1;
char *benchmarkName = NULL;
char *savePositions = NULL;
char *device = NULL;

int handle_args(int argc, char *argv[], jobType *preconditions, jobType *j,
                size_t *minSizeInBytes, size_t *maxSizeInBytes, size_t *timetorun, size_t *dumpPositions, size_t *defaultqd,
                unsigned short *seed, diskStatType *d, size_t *verify, double *timeperline, double *ignorefirst,
                char **mysqloptions, char **mysqloptions2, char *commandstring, char **filePrefix, int* doNumaBinding, int *performPreDiscard, int *reportMode)
{
  int opt;

  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0;
  size_t tripleX = 0;
  size_t commandstringpos = 0;
  size_t added = 0;
  *performPreDiscard = 0;
  *reportMode = 0;

  jobInit(j);
  jobInit(preconditions);

  optind = 0;
  size_t jglobalcount = 1;

  const char *getoptstring = "j:b:c:f:F:G:t:d:VB:I:q:XR:p:O:s:i:vP:M:N:e:uU:Tr";

  while ((opt = getopt(argc, argv, getoptstring )) != -1) {
    switch (opt) {
    case 'j':
      jglobalcount = atoi(optarg);
      fprintf(stderr,"*DEPRECATED, WILL BE REMOVED SOON* please move the j option into the -c command string\n");
      break;
    }
  }
  optind = 0;

  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'b':
    {}
    *minSizeInBytes = alignedNumber(atol(optarg), 4096);
    *maxSizeInBytes = alignedNumber(atol(optarg), 4096);
    if (*minSizeInBytes == *maxSizeInBytes) {
      *minSizeInBytes = 0;
    }
    if (*minSizeInBytes > *maxSizeInBytes) {
      fprintf(stderr,"*error* low range needs to be lower [%zd, %zd]\n", *minSizeInBytes, *maxSizeInBytes);
      exit(1);
    }
    break;

    case 'B':
      benchmarkName = strdup(optarg);
      break;
    case 'c':
    {}
    if (commandstringpos > 0) {
      commandstring[commandstringpos++] = ' ';
    }
      //      strncpy(commandstring + commandstringpos, optarg, strlen(optarg));
    memcpy(commandstring + commandstringpos, optarg, strlen(optarg));
    commandstringpos += strlen(optarg);
    commandstring[commandstringpos] = 0;

    char *charJ = strchr(optarg, 'j');

    int addthej = jglobalcount;
    if (charJ) {
      //	fprintf(stderr,"has a j\n");
      addthej = 0;
    }

    int joption = 0;
    if (charJ && *(charJ+1)) {
      joption = atoi(charJ + 1);
      if (joption < 1) joption = 1;
    }
    size_t jcount = jglobalcount; // global is the default
    if (joption) jcount = joption; // overwritten by an option

    if (verbose && jcount > 1) {
      fprintf(stderr,"*info* adding command '%s' x %zd times\n", optarg, jcount);
    }

    for (size_t i = 0; i < jcount; i++) {
      char temp[1000];
      if (addthej) {
        sprintf(temp,"%sj%zd#%zd", optarg, jcount, i);
      } else {
        sprintf(temp,"%s#%zd", optarg, i);
      }
      //	fprintf(stderr,"Adding %s\n", temp);
      jobAdd(j, temp);
    }
    break;
    case 'd':
      *dumpPositions = atoi(optarg);
      break;
    case 'e':
    {}
    double delay = atof(optarg);
    jobAddExec(j, optarg, delay);
    break;
    case 'i':
      *ignorefirst = atof(optarg) * 1024.0 * 1024.0 * 1024.0;
      fprintf(stderr,"*info* ignoring first %.1lf GiB of the test\n", TOGiB(*ignorefirst));
      break;
    case 'I':
    {}
    added = loadDeviceDetails(optarg, &deviceList, &deviceCount);
    fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
    break;
    case 'f':
      device = strdup(optarg);
      break;
    case 'j':
      break;
    case 'F':
      *filePrefix = strdup(optarg);
      addDeviceDetails(optarg, &deviceList, &deviceCount);
      break;
    case 'G':
    {}
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
    case 'M':
      *mysqloptions = strdup(optarg);
      break;
    case 'N':
      *mysqloptions2 = strdup(optarg);
      break;
    case 'O':
      diskStatFromFilelist(d, optarg, 0);
      //      fprintf(stderr,"*info* raw devices for amplification analysis: %zd\n", d->numDevices);
      break;
    case 'p': // pre-conditions
      jobAdd(preconditions, optarg);
      break;
    case 'P':
      savePositions = optarg;
      fprintf(stderr,"*info* savePositions set to '%s'\n", savePositions);
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
    case 'r':
      *reportMode = 1;
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
    case 'T':
      *performPreDiscard = 1;
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
    case 'U':
      if( *doNumaBinding == -2 ) {
        fprintf( stderr, "Cannot bind to NUMA if NUMA binding is disabled\n" );
        exit(1);
      }
      *doNumaBinding = atoi(optarg);
      if( *doNumaBinding < 0 ) {
        fprintf( stderr, "NUMA node invalid: NUMA nodes must be >= 0 \n" );
        exit( 1 );
      }
      break;
    case 'u':
      if( *doNumaBinding >= 0 ) {
        fprintf( stderr, "Cannot bind to NUMA if NUMA binding is disabled\n" );
        exit(1);
      }
      *doNumaBinding = -2;
      break;
    default:
      //      exit(1);
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
  } else if ( ((*filePrefix) != NULL) && (deviceCount < 1) ) {
    fprintf(stderr,"*error* no device specified\n");
    exit(1);
  }

  if (deviceCount) {
    // scale up based on the -I list
    jobMultiply(j, 1, deviceList, deviceCount);
    jobMultiply(preconditions, 1, deviceList, deviceCount);
  }

  // scale up using the -j
  /*  if (extraparalleljobs) {
    if (added) {
      fprintf(stderr,"*warning* it's unusual to add -j along with -I. This is probably wrong.\n");
    }
    jobMultiply(j, extraparalleljobs, NULL, 0);
    }*/

  if (added) {
    for (size_t i = 0; i < jobCount(j); i++) {
      if (strstr(j->strings[i], "G_")) {
        fprintf(stderr,"*warning* it's very weird to add G_ with -I. dev: %s, %s. This is probably wrong.\n", j->devices[i], j->strings[i]);
      }
    }
  }

  //  jobDump(j);

  if (*filePrefix) { // update prefix to be prefix.1..n
    jobFileSequence(j);
  }

  // check the file, create or resize
  size_t fsize = 0;
  for (size_t i = 0; i < jobCount(j); i++) {
    device = j->devices[i];
    size_t isAFile = 0;

    if (!fileExists(device)) { // nothing is there, create a file
      //fprintf(stderr,"*warning* will need to create '%s'\n", device);
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

    if (*filePrefix == NULL) {
      fsize = fileSizeFromName(device);
      if (isAFile) {
        if (*maxSizeInBytes == 0) { // if not specified use 2 x RAM
          *maxSizeInBytes = totalRAM() * 2;
        }
        if (fsize != *maxSizeInBytes) { // check the on disk size
          int ret = createFile(device, *maxSizeInBytes);
          if (ret) {
            exit(1);
          }
        }
      } else {
        // if you specify -G too big or it's 0 then set it to the existing file size
        if (*maxSizeInBytes > fsize || *maxSizeInBytes == 0) {
          if (*maxSizeInBytes > fsize) {
            fprintf(stderr,"*warning* limiting size to %zd, ignoring -G\n", *maxSizeInBytes);
          }
          *maxSizeInBytes = fsize;
          if (*minSizeInBytes > fsize) {
            fprintf(stderr,"*warning* limiting size to %d, ignoring -G\n", 0);
            *minSizeInBytes = 0;
          }
        }
      } // i
    } // fileprefix == 0
  }


  if (verbose) jobDump(j);

  for(size_t i = 0; i < deviceCount; i++ ) {
    free( deviceList[ i ].devicename );
  }

  return 0;
}

void usage()
{
  fprintf(stdout,"\nUsage:\n  spit [-f device] [-c string] [-c string] ... [-c string]\n");
  fprintf(stdout,"\nExamples:\n");
  fprintf(stdout,"  spit -f device -c ... -c ... -c ... # defaults to %d seconds\n", DEFAULTTIME);
  fprintf(stdout,"  spit -f device -c r           # seq read (defaults to s1 and k4)\n");
  fprintf(stdout,"  spit -f device -c w           # seq write (s1)\n");
  fprintf(stdout,"  spit -f device -c rs0         # random, (s)equential is 0\n");
  fprintf(stdout,"  spit -f device -c rs0-size    # random, (s)equential is 0, max contig is size KiB\n");
  fprintf(stdout,"  spit -f device -c ws128       # 128 parallel (s)equential writes\n");
  fprintf(stdout,"  spit -f device -c rs128P1000  # 128 parallel writes, 1000 positions\n");
  fprintf(stdout,"  spit -f device -c k8          # set block size to 8 KiB\n");
  fprintf(stdout,"  spit -f device -c k4-128      # set block range to 4 to 128 KiB, every 4 KiB\n");
  fprintf(stdout,"  spit -f device -c k4:1024     # set block range to 4 to 1024 KiB, in powers of 2\n");
  fprintf(stdout,"  spit -f device -c W5          # do 5 seconds worth of IO then wait for 5 seconds\n");
  fprintf(stdout,"  spit -f device -c W0.1:4      # do 0.1 seconds worth of IO then wait for 4 seconds\n");
  fprintf(stdout,"  spit -f ... -c w -cW4rs0      # one thread seq write, one thread, run 4, wait 4 repeat\n");
  fprintf(stdout,"  spit -f device -c \"r s128 k4\" -c \'w s4 -k128\' -c rw\n");
  fprintf(stdout,"  spit -f device -c r -G 1      # 1 GiB device size\n");
  fprintf(stdout,"  spit -f device -b 10240000    # specify the max device size in bytes\n");
  fprintf(stdout,"  spit -f device -c r -G 1-2    # Only perform actions in the 1-2 GiB range\n");
  fprintf(stdout,"  spit -c ws1G1-2 -c rs0G2-3    # Seq w in the 1-2 GiB region, rand r in the 2-3 GiB region\n");
  fprintf(stdout,"  spit -f ... -t 50             # run for 50 seconds (-t -1 is forever)\n");
  fprintf(stdout,"  spit -f ... -j 32             # duplicate all the commands 32 times. If available, distribute & pin threads to each NUMA node.\n");
  fprintf(stdout,"  spit -f ... -j 32 -u          # duplicate all the commands 32 times, but do not pin the threads to specific NUMA nodes\n");
  fprintf(stdout,"  spit -f ... -j 32 -U 0        # duplicate all the commands 32 times, pin all threads to  NUMA node 0\n");
  fprintf(stdout,"  spit -f ... -f ...-d 10       # dump the first 10 positions per command\n");
  fprintf(stdout,"  spit -f ... -c rD0            # 'D' turns off O_DIRECT\n");
  fprintf(stdout,"  spit -f ... -c wR42           # set the per command seed with R\n");
  fprintf(stdout,"  spit -f ... -c wF             # (F)lush after every write of FF for 10, FFF for 100 ...\n");
  fprintf(stdout,"  spit -f ... -c rrrrw          # do 4 reads for every write\n");
  fprintf(stdout,"  spit -f ... -c rw             # mix 50/50 reads/writes\n");
  fprintf(stdout,"  spit -f ... -c n              # shuffles the positions every pass\n");
  fprintf(stdout,"  spit -f ... -c N              # adds an offset to the positions every pass\n");
  fprintf(stdout,"  spit -f ... -t -1             # -t -1 is run forever\n");
  fprintf(stdout,"  spit -f ... -c p0.9           # set the r/w ratio to 0.9\n");
  fprintf(stdout,"  spit -f ... -c wz             # sequentially (w)rite from block (z)ero (instead of random position)\n");
  fprintf(stdout,"  spit -f ... -c m              # non-unique positions, read/write/flush like (m)eta-data\n");
  fprintf(stdout,"  spit -f ... -c mP4000         # non-unique 4000 positions, read/write/flush like (m)eta-data\n");
  fprintf(stdout,"  spit -f ... -c s1n            # do a sequential pass, then shuffles the positions\n");
  fprintf(stdout,"  spit -f ... -c rL4            # (L)imit positions so the sum of the length is 4 GiB\n");
  fprintf(stdout,"  spit -f ... -c P10G1-2        # The first 10 positions starting from 1GiB. It needs the lower range.\n");
  fprintf(stdout,"  spit -f ... -c P10x100        # multiply the number of positions by x, here it's 100\n");
  fprintf(stdout,"  spit -f ... -c wM1            # set block size 1M\n");
  fprintf(stdout,"  spit -f ... -c O              # One-shot, not time based\n");
  fprintf(stdout,"  spit -f ... -c t2             # specify the time per thread\n");
  fprintf(stdout,"  spit -f ... -c ws1J4          # jumble/reverse groups of 4 positions\n");
  fprintf(stdout,"  spit -f ... -c wx2O           # sequentially (s1) write 200%% LBA, no time limit\n");
  fprintf(stdout,"  spit -f ... -c ws0            # random defaults to 3x LBA\n");
  fprintf(stdout,"  spit -f ... -c ws1W2:1 -t60   # Alternate run for 2 seconds, wait for 1 second\n");
  fprintf(stdout,"  spit -I devices.txt -c r      # -I is read devices from a file\n");
  fprintf(stdout,"  spit -f .... -R seed          # set the initial seed, -j will increment per job\n");
  fprintf(stdout,"  spit -f .... -Q qd            # set the per job default queue depth\n");
  fprintf(stdout,"  spit -f .... -c q128          # per job queue\n");
  fprintf(stdout,"  spit -f .... -c Q17           # per job queue depth, square wave/burst, 17 submits then 17 returns.\n");
  fprintf(stdout,"  spit -f .... -c wns0X10       # writing the number of positions 10 times, not time based\n");
  fprintf(stdout,"  spit -c x5                    # writing the block device size 5 times, not time based\n");
  fprintf(stdout,"  spit -c P10x1                 # write 10 positions until the entire device size is written\n");
  fprintf(stdout,"  spit -c P10X1                 # write 10 positions\n");
  fprintf(stdout,"  spit -c P10000X100            # write the same 10,000 positions 100 times\n");
  fprintf(stdout,"  spit -c P10000                # write the same 10,000 positions based on the time\n");
  fprintf(stdout,"  spit -p G -p Gs1              # precondition job, writing random, 100%% LBA, then seq job\n");
  fprintf(stdout,"  spit -c wZ1                   # Z is the starting offset. -z is -Z0\n");
  fprintf(stdout,"  spit -p G100                  # precondition job, writing random overwrite LBA size\n");
  fprintf(stdout,"  spit -p G100s1k64             # precondition job, sequential, 64 KiB blocks\n");
  fprintf(stdout,"  spit -f meta -O devices.txt   # specify the raw devices for amplification statistics\n");
  fprintf(stdout,"  spit -s 0.1 -i 5              # and ignore first 5 GiB of performance\n");
  fprintf(stdout,"  spit -v                       # verify the writes after a run\n");
  fprintf(stdout,"  spit -P filename              # dump positions to filename\n");
  fprintf(stdout,"  spit -c wG_j4                 # The _ represents to divide the G value evenly between threads\n");
  fprintf(stdout,"  spit -B bench -M ... -N ...   # See the man page for benchmarking tips\n");
  fprintf(stdout,"  spit -F fileprefix -j128      # creates files from .0001 to .0128\n");
  fprintf(stdout,"  spit ... -c ws0u -v           # Uses a unique seed (u) per operation (mod 65536)\n");
  fprintf(stdout,"  spit ... -c ws0U -v           # Generates a read immediately after a write (U), tests with QD=1\n");
  fprintf(stdout,"  spit ... -c ws0UG_j32 -v      # Generates r/w pairs with unique seeds, as above, unique thread ranges\n");
  fprintf(stdout,"  spit ... -c ws1S250           # S option targets a speed in MB/s by adding usleep() between operations. Low speeds only\n");
  fprintf(stdout,"  spit -e \"5,echo five\"         # exec a bash -c CMD string after 5 seconds, quotes are required\n");
  fprintf(stdout,"  spit -c wk1024za7             # every 'a' MiB of operations perform a jump back to the start of device. Dump with -d to see\n");
  fprintf(stdout,"  spit -c wk1024za3A8           # 'A' means to add 8 KiB after every position after 3 MiB\n");
  fprintf(stdout,"  spit -c ws1G5_10j16           # specify a low and high GiB range, to be evenly split by 16 threads (_)\n");
  fprintf(stdout,"  spit -c wx3 -G4 -T            # perform pre-DISCARD/TRIM operations before each write iteration\n");
  fprintf(stdout,"  spit -c ts0                   # Use a sync DISCARD/TRIM I/O type\n");
  fprintf(stdout,"  spit -c rrwts0                # 50%% read, 25%% writes and 25%% trim I/O random operations\n");
  exit(0);
}


void intHandler(int d)
{
  if (d) {}
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}


void doReport(size_t timetorun, size_t maxSizeInBytes) {

  if (!device) {
    fprintf(stderr,"*error* no -f device provided\n");
    return;
  }

  if (!canOpenExclusively(device)) {
    fprintf(stderr,"*error* device already open!\n");
    return;
  }

  if (maxSizeInBytes == 0) maxSizeInBytes = 50 * 1024 * 1024;

  char text[100];
  time_t now = time(NULL);
  struct tm *t = localtime(&now);
  
  strftime(text, sizeof(text)-1, "%Y-%m-%d", t);
 
  
  fprintf(stdout, "== Report: `stutools: spit`\n");

  unsigned int major, minor;
  majorAndMinorFromFilename(device, &major, &minor);

  char *suffix = getSuffix(device);
  
  fprintf(stdout, " Device: %s (major %d, minor %d)\n", device, major, minor);
  size_t bdsize = fileSizeFromName(device);
  char *model = getModel(suffix);
  fprintf(stdout, " Model: %s\n", model);
  if (model) free(model);
  model = NULL;
  fprintf(stdout, " Device size: %.3lf GB\n", TOGB(bdsize));
  fprintf(stdout, " Testing size: %.3lf GB (%.3lf %% of device)\n", TOGB(maxSizeInBytes), 100.0 * maxSizeInBytes / bdsize);
  char *host = hostname();

  size_t discard_max_bytes, discard_granularity, discard_zeroes_data, alignment_offset;
  getDiscardInfo(suffix, &alignment_offset, &discard_max_bytes, &discard_granularity, &discard_zeroes_data);
  fprintf(stdout, " Trim/Discard: alignment_offset %zd, max_bytes %zd (%.1lf MiB), granularity %zd, zeroes_data %zd\n", alignment_offset, discard_max_bytes, TOGiB(discard_max_bytes), discard_granularity, discard_zeroes_data);
  if (suffix) free(suffix);
  suffix = NULL;

  
  fprintf(stdout, " Machine: %s\n", host);
  char *cpumodel = getCPUModel();
  fprintf(stdout, " CPU: %s\n", cpumodel);
  if (cpumodel) free(cpumodel);
  if (host) free(host);
  fprintf(stdout, " NUMA nodes: %d\n", getNumaCount());
  fprintf(stdout, " Thread count: %d\n", getNumHardwareThreads());
  char *os = OSRelease();
  fprintf(stdout, " OS: %s\n", os);
  if (os) free(os);
  fprintf(stdout, " RAM: %.0lf GiB\n", TOGiB(totalRAM()));
  fprintf(stdout, " Report date: %s\n", text);
  fprintf(stdout, " stutools: %s\n", VERSION);
  fprintf(stdout, "\n\n");
  
  diskStatType d;
  diskStatSetup(&d);

  size_t fsize = MIN(maxSizeInBytes, fileSizeFromName(device)); // the first xx MiB

  if (fsize <= 0) {
    fprintf(stderr, "*error* no file called '%s'\n", device);
    return;
  }

  size_t blockSize1[] = {4,4,8,16,8, 32,64,128,256,128,512,1024,2048};
  size_t blockSize2[] = {4,8,8,16,64,32,64,128,256,512,512,1024,2048};

  size_t threadBlock[] = {64};

  size_t dedupSizes[] = {1,10,100,1000,10000,100000,1000000};
  char s[100];
  jobType j;

  double starttime = timedouble();
  int round = 0;
  resultType r;
  size_t xcopies = 1;
  
  while (timedouble() - starttime < timetorun) {
    fprintf(stdout, "=== Round %d\n\n", ++round);


    fprintf(stdout, "==== Random Write\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
	  sprintf(s, "w s0 k%zd-%zd j%zd#%zd q16 G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , t2, xcopies);
	  jobAdd(&j, s); 
	}
	jobAddDeviceToAll(&j, device);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 4, 42, 0, NULL /* diskstats &d*/, 0.1, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0,  &r);
	sprintf(s, "w s0 k%zd-%zd j%zd G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , xcopies);
	fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);


    fprintf(stdout, "==== Sequential Write\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
	  sprintf(s, "w s1 k%zd-%zd j%zd#%zd q16 G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , t2, xcopies);
	  jobAdd(&j, s); 
	}
	jobAddDeviceToAll(&j, device);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 4, 42, 0, NULL /* diskstats &d*/, 0.1, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0,  &r);
	sprintf(s, "w s1 k%zd-%zd j%zd G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , xcopies);
	fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);



    fprintf(stdout, "==== Random Read\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
	  sprintf(s, "r s0 k%zd-%zd j%zd#%zd q64 G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , t2, xcopies);
	  jobAdd(&j, s); // x1 is LBA, X1 should be 100
	}
	jobAddDeviceToAll(&j, device);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 16, 42, 0, NULL /* diskstats &d*/, 0.1, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0,  &r);
	sprintf(s, "r s0 k%zd-%zd j%zd G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , xcopies);
	fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);



    fprintf(stdout, "==== Sequential Read\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
	  sprintf(s, "r s1 k%zd-%zd j%zd#%zd q64 G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , t2, xcopies);
	  jobAdd(&j, s); // x1 is LBA, X1 should be 100
	}
	jobAddDeviceToAll(&j, device);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 16, 42, 0, NULL /* diskstats &d*/, 0.1, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0,  &r);
	sprintf(s, "r s1 k%zd-%zd j%zd G_ x%zd", blockSize1[i], blockSize2[i], threadBlock[t] , xcopies);
	fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);


    fprintf(stdout, "==== Write de-dup\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(dedupSizes) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t jj = 0 ; jj < threadBlock[t]; jj++) {
	  sprintf(s, "w s0 P%zd j%zd#%zd q16 G_ k4 x3", dedupSizes[i], threadBlock[t], jj);
	  jobAdd(&j, s); 
	}
	jobAddDeviceToAll(&j, device);
	sprintf(s, "w s0 P%zd j%zd q16 G_ k4 x3", dedupSizes[i], threadBlock[t]);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 4, 42, 0, NULL /* diskstats &d*/, 0.1, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0,  &r);
	fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);


    fprintf(stdout, "==== Random Write metadata + read\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command |  Threads | Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t jj = 0 ; jj < threadBlock[t]; jj++) {
	  sprintf(s, "w s0 k%zd P100000 q2 j%zd#%zd G_ x%d", blockSize1[i], threadBlock[t], jj, 1);
	  jobAdd(&j, s); 
	  sprintf(s, "r s0 k%zd P100000 q2 j%zd#%zd G_ x%d", blockSize2[i], threadBlock[t], jj, 1);
	  jobAdd(&j, s); 
	}
	
	jobAddDeviceToAll(&j, device);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 32, 42, NULL /* save positions*/ , NULL /* diskstats &d*/, 0.1 /*timeline*/, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0, &r);
	sprintf(s, "r s0 k%zd P100000 q2 j%zd G_ / +w k%zd", blockSize1[i], threadBlock[t], blockSize2[i]);
	fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);


    //
    fprintf(stdout, "==== Random 70%% Read / 30%% Write\n\n");
    fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1,^2\", options=\"header\"]\n");
    fprintf(stdout, "|===\n");
    fprintf(stdout, "| Command | Threads | Read IOPS | Write IOPS | Read GB/s | Write GB/s | Range (GiB)\n");
    

    for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
      for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
	jobInit(&j);
	for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
	  sprintf(s, "p0.7 s0 k%zd-%zd G_ j%zd#%zd x%zd", blockSize1[i], blockSize2[i], threadBlock[t], t2, xcopies);
	  jobAdd(&j, s);
	}
	jobAddDeviceToAll(&j, device);
	size_t low = 0, high = fsize;
	if (fsize * (i+1) <= bdsize) {
	  low = fsize * i;
	  high = fsize * (i+1);
	}
	jobRunThreads(&j, j.count, NULL, low, high, 3, 0, NULL, 32, 42, NULL /* save positions*/ , NULL /* diskstats &d*/, 0.1 /*timeline*/, 0, 1 /*verify*/, NULL, NULL, NULL, -1, 0, &r);
	sprintf(s, "p0.7 s0 k%zd-%zd G_ j%zd x%zd", blockSize1[i], blockSize2[i], threadBlock[t], xcopies);
	fprintf(stdout, "| %s | %zd  |   %.0lf | %.0lf |  %.1lf |  %.1lf | %.1lf–%.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, r.readMBps, r.writeMBps, TOGiB(low), TOGiB(high));
	fflush(stdout);
      }
    }
    fprintf(stdout, "|===\n\n");
    fflush(stdout);

  }

  diskStatFree(&d);
}


/**
 * main
 *
 */
int main(int argc, char *argv[])
{

  size_t fuzz = 0, runcount = 0;
  char *fuzzdevice = NULL;
  if (argc == 1) {
    usage();
  } else if (argc > 2) {
    fuzz = (strcmp(argv[1],"fuzz") == 0);
    if (fuzz) fuzzdevice = argv[2];
  }

  // set OOM adjust to 1,000 to make this program be killed first
  FILE *fp = fopen("/proc/self/oom_score_adj", "wt");
  fprintf(fp,"1000\n");
  fclose(fp);


  double starttime = timedouble();

  fprintf(stderr,"*info* spit %s %s (Stu's powerful I/O tester)\n", argv[0], VERSION);

  if (swapTotal() > 0) {
    fprintf(stderr,"*warning* spit needs swap to be off for consistent numbers. `sudo swapoff -a`\n");
  }

  char **argv2 = NULL;
  int argc2;

  char *filePrefix = NULL;

  do {
    jobType *j = malloc(sizeof(jobType));
    jobType *preconditions = malloc(sizeof(jobType));

    if (fuzz) {
      verbose = 0;
      argv2 = fuzzString(&argc2, fuzzdevice, starttime, &runcount);
    } else {
      argc2 = argc;
      argv2 = argv;
    }

    size_t defaultQD = 16;
    unsigned short seed = 0;
    diskStatType d;
    size_t verify = 0;
    double timeperline = 1, ignoreFirst = 0;
    int doNumaBinding = -1; // -1 default, -2 disable, >= 0 bind to specific numa
    int performPreDiscard = 0;
    int reportMode = 0;

    diskStatSetup(&d);
    size_t minSizeInBytes = 0, maxSizeInBytes = 0, timetorun = DEFAULTTIME, dumpPositions = 0;
    char *mysqloptions = NULL, *mysqloptions2 = NULL;

    char commandstring[1000];
    handle_args(argc2, argv2, preconditions, j, &minSizeInBytes, &maxSizeInBytes, &timetorun, &dumpPositions, &defaultQD, &seed, &d, &verify, &timeperline, &ignoreFirst, &mysqloptions, &mysqloptions2, commandstring, &filePrefix, &doNumaBinding, &performPreDiscard, &reportMode);

    if (reportMode) {
      doReport(timetorun, maxSizeInBytes);
    } else if (j->count < 1) {
      fprintf(stderr,"*error* missing -c command options\n");
    } else { // run some jobs
      printPowerMode();

      size_t actualSize = maxSizeInBytes - minSizeInBytes;
      fprintf(stderr,"*info* block range [%.2lf-%.2lf] GB, size %.2lf GB (%zd bytes). Range [%.3lf-%.3lf] TB\n", TOGB(minSizeInBytes), TOGB(maxSizeInBytes), TOGB(actualSize), actualSize, TOTB(minSizeInBytes), TOTB(maxSizeInBytes));
      if (actualSize < 4096) {
	fprintf(stderr,"*error* block device too small.\n");
	exit(1);
      }

      if (preconditions) {
	keepRunning = 1;
	signal(SIGTERM, intHandler);
	signal(SIGINT, intHandler);
	jobRunPreconditions(preconditions, preconditions->count, minSizeInBytes, maxSizeInBytes);
      }

      keepRunning = 1;
      diskStatType *p = &d;
      if (!d.allocDevices) {
	p = NULL;
      }
      signal(SIGTERM, intHandler);
      signal(SIGINT, intHandler);

      jobRunThreads(j, j->count, filePrefix, minSizeInBytes, maxSizeInBytes, timetorun, dumpPositions, benchmarkName, defaultQD, seed, savePositions, p, timeperline, ignoreFirst, verify, mysqloptions, mysqloptions2, commandstring, doNumaBinding, performPreDiscard, NULL);

      diskStatFree(&d);

      if (fuzz) {
	for (int i = 0; i < argc2; i++) {
	  free(argv2[i]);
	  argv2[i] = NULL;
	}
	free(argv2);
	argv2 = NULL;
      }
      if (mysqloptions) {
	free(mysqloptions);
	mysqloptions = NULL;
      }
      if (mysqloptions2) {
	free(mysqloptions2);
	mysqloptions2 = NULL;
      }

    } // end of job

    jobFree(j);
    free(j);

    jobFree(preconditions);
    free(preconditions);


    //    if (timedouble() - starttime > 3600) break;
  } while (fuzz);

  if (benchmarkName) free(benchmarkName);
  //  if (device) free(device);

  fprintf(stderr,"*info* exiting.\n");
  fflush(stderr);
  exit(0);
}


