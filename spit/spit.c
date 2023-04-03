#define _POSIX_C_SOURCE 200809L
#define _DEFAULT_SOURCE
#define _GNU_SOURCE

#include "jobType.h"
#include <signal.h>
#include <math.h>

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
#include <assert.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include <fcntl.h>

#include "positions.h"
#include "utils.h"
#include "diskStats.h"
#include "spitfuzz.h"

#define DEFAULTTIME 10

int verbose = 0;
int keepRunning = 1;
char *benchmarkName = NULL;
FILE *savePositions = NULL;
char *device = NULL;
char *logfile = NULL;
double lowg = 0, highg = 0;

int handle_args(int argc, char *argv[], jobType *preconditions, jobType *j,
                size_t *minSizeInBytes, size_t *maxSizeInBytes, double *runseconds, size_t *dumpPositions,
                unsigned short *seed, diskStatType *d, size_t *verify, double *timeperline, double *ignorefirst,
                char **mysqloptions, char **mysqloptions2, char *commandstring, char **filePrefix, char** numaBinding, int *performPreDiscard, int *reportMode,
                size_t *cacheSizeBytes, size_t *forever, size_t *ramBytesForPositions, size_t *tripleX, size_t *showdate, char **kcheckresults, FILE **filepos)
{
  int opt;

  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0;
  size_t commandstringpos = 0;
  size_t added = 0;
  *tripleX = 0;
  *forever = 0;
  *performPreDiscard = 0;
  *reportMode = 0;

  jobType jtemp;
  jobInit(&jtemp);

  jobType pretemp;
  jobInit(&pretemp);


  commandstring[0] = 0;

  optind = 0;
  size_t jglobalcount = 1;

  const char *getoptstring = "Ej:b:c:f:F:G:g:t:d:VB:I:XR:p:O:s:i:vP:M:N:e:uU:TrC:1L:DK:l:S:";

  while ((opt = getopt(argc, argv, getoptstring )) != -1) {
    switch (opt) {
    case 'K':
      // check
      if (kcheckresults) {
	*kcheckresults = strdup(optarg);
	fprintf(stderr,"*warning* check answers are '%s'\n", *kcheckresults);
      }
      break;
    case 'S':
      {}
      FILE *fp = fopen(optarg, "rt");
      if (fp) {
	fprintf(stderr,"*info* position/actions will be loaded from '%s'\n", optarg);
	*filepos = fp;
      } else {
	perror(optarg);
	*filepos = NULL;
      }
      break;
    case 'D':
      fprintf(stderr,"*info* showing date/time in output\n");
      *showdate = 1;
      break;
    case '1':
      *forever = 1;
      break;
    case 'j':
      jglobalcount = atoi(optarg);
      fprintf(stderr,"*DEPRECATED, WILL BE REMOVED SOON* please move the j option into the -c command string\n");
      break;
    }
  }
  optind = 0;
  int GBpow2 = 1;
  int GBpercent = 0;

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
    case 'C':
      *cacheSizeBytes = 1024L * 1024 * 1024 * atof(optarg);
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
    if (charJ == NULL) {
      charJ = strchr(optarg, 'J');
    }

    int addthej = jglobalcount;
    if (charJ) {
      //      fprintf(stderr,"has a %c\n", *charJ);
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
        sprintf(temp,"%s%c%zd#%zd", optarg, charJ ? *charJ : 'j', jcount, i);
      } else {
        sprintf(temp,"%s#%zd", optarg, i);
      }
      //	fprintf(stderr,"Adding %s\n", temp);
      jobAdd(&jtemp, temp);
    }
    break;
    case 'd':
      *dumpPositions = atoi(optarg);
      break;
    case 'e':
    {}
    double delay = atof(optarg);
    jobAddExec(&jtemp, optarg, delay);
    break;
    case 'i':
      *ignorefirst = atof(optarg) * 1024.0 * 1024.0 * 1024.0;
      fprintf(stderr,"*info* ignoring first %.1lf GiB of the test\n", TOGiB(*ignorefirst));
      break;
    case 'I':
    {}
    added = loadDeviceDetails(optarg, &deviceList, &deviceCount);
    if (added) fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
    break;
    case 'f':
      addDeviceDetails(optarg, &deviceList, &deviceCount);
      //      device = strdup(optarg);
      break;
    case 'j':
      break;
    case 'F':
      {
      /*      if (isBlockDevice(optarg)) {
	fprintf(stderr,"*error* '%s' can't be an existing block device, it's a mount point and prefix\n", optarg);
	exit(1);
	}e*/
      struct stat sb;
      int ret;
      
      if ((ret = stat(optarg, &sb)) != -1) {
	//    fprintf(stderr,"*warning* isBlockDevice '%s' returned %d\n", name, ret);
	switch (sb.st_mode & S_IFMT) { // check the filetype bits
	case S_IFDIR:
	  fprintf(stderr,"*error* must specify a path including the prefix\n");
	  exit(1);
	  break;
	}
      }
      
      *filePrefix = strdup(optarg);
      addDeviceDetails(optarg, &deviceList, &deviceCount);
      }
      break;
    case 'E':
      fprintf(stderr,"*warning* -G/g values are percents...0..100%%\n");
      GBpercent = 1;
      break;
    case 'g':
      GBpow2 = 0;
      // fall through
    case 'G':
      {}
     const size_t smallbdsize = smallestBDSize(deviceList, deviceCount);
     GBpow2 = 0;
     if (strchr(optarg, '-') == NULL) {
      // no range
       size_t num = 0;
       if (GBpercent) {
	 num = alignedNumber(atof(optarg) * smallbdsize / 100.0, 4096);
	 *minSizeInBytes = num;
	 *maxSizeInBytes = num;
       } else {
	 num = alignedNumber(stringToBytesDefault(optarg, GBpow2), 4096);
	 *minSizeInBytes = num;
	 *maxSizeInBytes = num;
       }
       lowg = atof(optarg); highg = atof(optarg);
       fprintf(stderr,"*info* -G option %zd bytes, power2=%d, (%.5lf GiB, %.5lf GB)\n", num, GBpow2, TOGiB(num), TOGB(num));
    } else { 
      splitRange(optarg, &lowg, &highg);
      if (GBpercent) {
	*minSizeInBytes = alignedNumber(lowg * smallbdsize / 100, 4096);
	*maxSizeInBytes = alignedNumber(highg * smallbdsize / 100, 4096);
      } else {
	*minSizeInBytes = alignedNumber(pow(GBpow2 ? 1024 : 1000, 3.0) * lowg, 4096);
	*maxSizeInBytes = alignedNumber(pow(GBpow2 ? 1024 : 1000, 3.0) * highg, 4096);
      }
      if ((highg == 0) && (lowg > 0)) {
	*maxSizeInBytes = smallbdsize;
      }
    }
    if (*minSizeInBytes == *maxSizeInBytes) {
      *minSizeInBytes = 0;
    }
    if (*minSizeInBytes > *maxSizeInBytes) {
      fprintf(stderr,"*error* low range needs to be lower [%zd, %zd]\n", *minSizeInBytes, *maxSizeInBytes);
      exit(1);
    }
    break;
    case 'l':
      logfile = strdup(optarg);
      fprintf(stderr,"*info* summary stats logged to file '%s'\n", logfile);
      break;
    case 'L':
      *ramBytesForPositions = 1024L * 1024L * 1024L * atof(optarg);
      fprintf(stderr,"*info* limit RAM use to %.2lf GiB\n", TOGiB(*ramBytesForPositions));
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
      jobAdd(&pretemp, optarg);
      break;
    case 'P':
      if (strcmp(optarg, "-")==0) {
	savePositions = stdout;
	struct stat bf;
	fstat(fileno(savePositions),  &bf);
	if (S_ISFIFO(bf.st_mode)) {
	  fcntl(fileno(savePositions), F_SETPIPE_SZ, 0);
	  int ld = fcntl(fileno(savePositions), F_GETPIPE_SZ);
	  fprintf(stderr,"*info* positions streamed to FIFO/pipe (buffer %d)\n", ld);
	} else {
	  fprintf(stderr,"*info* positions streamed to (stdout)\n");
	  setlinebuf(savePositions);
	}
      } else {
	savePositions = fopen(optarg, "wt");
	if (!savePositions) {perror(optarg); exit(-1);}
	fprintf(stderr,"*info* savePositions set to '%s'\n", optarg);
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
      *runseconds = atof(optarg);
      if (*runseconds == 0) {
        fprintf(stderr,"*error* zero isn't a valid time. -t -1 for a long time\n");
        exit(1);
        //	*runseconds = (size_t)-1; // run for ever
      }
      if (*runseconds < 0) {
        *runseconds = INF_SECONDS; 
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
      (*tripleX)++;
      break;
    case 'U':
      *numaBinding = strdup( optarg );
      break;
    case 'u':
      *numaBinding = "";
      break;
    default:
      //      exit(1);
      break;
    }
  }

  // first assign the device
  // if one -f specified, add to all jobs
  // if -F specified (with n drives), have c x n jobs
  /*  if (deviceCount) {
    // if we have a list of files, overwrite the first one, ignoring -f
    if (device) {
      fprintf(stderr,"*warning* ignoring the value from -f\n");
    }
    device = deviceList[0].devicename;
    }*/

  /*
  if (device) {
    // set the first device to all of the jobs
    jobAddDeviceToAll(j, device);
    jobAddDeviceToAll(preconditions, device);
  } else if ( ((*filePrefix) != NULL) && (deviceCount < 1) ) {
    fprintf(stderr,"*error* no device specified\n");
    exit(1);
  }
  */

  //  fprintf(stderr,"device count: %zd\n", deviceCount);
  //  fprintf(stderr,"job count: %d\n", jtemp.count);
  //  fprintf(stderr,"pre count: %d\n", pretemp.count);

  jobInit(j);
  jobInit(preconditions);

  if (deviceCount) {
    // scale up based on the -I list
    // multiply j x deviceList
    jobMultiply(j, &jtemp, deviceList, deviceCount);
    jobMultiply(preconditions, &pretemp, deviceList, deviceCount);
  }

  if (deviceCount > 0) {
    device = strdup(deviceList[0].devicename);
  } else {
    if (deviceCount == 0) {
      fprintf(stderr,"*error* no devices specified\n");
    }
    if (jtemp.count == 0) {
      fprintf(stderr,"*error* no jobs specified\n");
    }
    exit(-1);
  }



  //  jobDump(j);

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
  for (size_t i = 0; i < jobCount(j); i++) {
    //    fprintf(stderr,"checking %s ...\n", j->devices[i]);
    size_t isAFile = 0;

    if (!fileExists(j->devices[i])) { // nothing is there, create a file
      //fprintf(stderr,"*warning* will need to create '%s'\n", device);
      isAFile = 1;
    } else {
      // it's there
      if (isBlockDevice(j->devices[i]) == 2) {
        // it's a file
        isAFile = 1;
      }

      if ((*tripleX) < 3) {
        if (!canOpenExclusively(j->devices[i])) {
	  perror(device);
          fprintf(stderr,"*error* can't open '%s' exclusively\n", device);
          exit(-1);
        }
      }
    }

    size_t fsize = 0;
    if (isAFile) {
      
      if (*maxSizeInBytes == 0) { // if not specified use 2 x RAM
	fprintf(stderr,"*warning* -Gn not specified so defaulting filesize to 2x RAM\n");
	*maxSizeInBytes = totalRAM() * 2;
      }

      if (*maxSizeInBytes == fileSizeFromName(j->devices[i])) {
	if (*filePrefix) {
	  fprintf(stderr,"*warning* file '%s' already exists. Leaving in place. For benchmarking writes remove first\n", j->devices[i]);
	}
      }

      if (*filePrefix == NULL) {
	int ret = createFile(j->devices[i], *maxSizeInBytes); // create file, but if already there then don't recreate
	if (ret) {
	  exit(1);
	}
      }
    } else {
      fsize = fileSizeFromName(j->devices[i]);
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
    } 
  }

  //  for (size_t i = 0; i < jobCount(j); i++) {
  //    fprintf(stderr,"*info* check '%s' size %zd\n", j->devices[i], fileSizeFromName(j->devices[i]));
  //  }
  //  exit(1);

  freeDeviceDetails(deviceList, deviceCount);
  if (verbose) jobDump(j);

  jobFree(&jtemp);

  //  for(size_t i = 0; i < deviceCount; i++ ) {
  //    free( deviceList[ i ].devicename );
  //  }

  return 0;
}

void usage()
{
  fprintf(stdout,"\nUsage:\n  spit [-f device] [-c string] [-c string] ... [-c string]\n");
  fprintf(stdout,"\nCreate positions:\n");
  fprintf(stdout,"  spit -f dev -c rs0            # defaults to use RAM and generate unique positions. MIN(15GB,freeRAM/2)\n");
  fprintf(stdout,"  spit -f dev -c rs1P100z       # the first 100 positions in a device, starting from zero\n");
  fprintf(stdout,"  spit -f dev -c rs1P100        # the first 100 positions in a device, offset by a random amount\n");
  fprintf(stdout,"  spit -f dev -c rs0P100        # the first 100 positions in a device, randomised\n");
  fprintf(stdout,"  spit -f dev -c rs1P+100z      # 100 positions equally spaced across the array, starting from zero\n");
  fprintf(stdout,"  spit -f dev -c rs1P+100       # 100 positions equally spaced across the array, offset by a random amount\n");
  fprintf(stdout,"  spit -f dev -c rs0P+100       # 100 positions equally spaced, randomised\n");
  fprintf(stdout,"  spit -f dev -c rs1P-100       # subsample with replacement, 100 positions randomly distributed over the array\n");
  fprintf(stdout,"  spit -f dev -c rs0P-100       # effectively the same as above with s1. But re-randomised.\n");
  fprintf(stdout,"  spit -f dev -c rs1P.100z      # 100 equally spaced, alternating start,end,s+1,e-1... centre\n");
  fprintf(stdout,"  spit -f dev -c rs1P.100       # effectively the same as above with s1 but with random offset\n");
  fprintf(stdout,"  spit -f dev -c rs0 -L20       # Use 20GiB of RAM to generate more unique positions\n");
  fprintf(stdout,"  spit -f dev -c rs0j2G_        # Make two threads, but first split the LBA range into two.\n");
  
  fprintf(stdout,"\nCopying regions:\n");
  fprintf(stdout,"  spit -f dev -c c              # will copy from a region to a destination 1/2 the LBA away, 4 KiB blocks\n");
  fprintf(stdout,"  spit -f dev -c ck64           # will copy using 64 KiB blocks\n");
  fprintf(stdout,"  spit -f dev -c ck64j4G_       # Divides the space into 4 regions, runs 4 threads performing local copies per region\n");
  fprintf(stdout,"  spit -f dev -c ckz64-2048      # Will perform 2 MiB worth of 64 KiB reads, then a single 2 MiB write, starting from 0\n");

  fprintf(stdout,"\nVarying executions: (time, position and LBA controls)\n");
  fprintf(stdout,"  spit -f dev -c rs0            # run for 10 seconds by default\n");
  fprintf(stdout,"  spit -f dev -c rs0 -t10       # run for 10 seconds\n");
  fprintf(stdout,"  spit -f dev -c rs0 -t-1       # run for ever\n");
  fprintf(stdout,"  spit -f dev -c rs0x1          # cover the entire LBA one time\n");
  fprintf(stdout,"  spit -f dev -c rs0x2          # cover the entire LBA two times\n");
  fprintf(stdout,"  spit -f dev -c rs0P100x1      # perform a LBA worth of those first 100 positions\n");
  fprintf(stdout,"  spit -f dev -c rs0P100x3      # perform 3xLBA worth of those first 100 positions\n");
  fprintf(stdout,"  spit -f dev -c rs0P2000X1     # execute the 2,000 IO operations\n");
  fprintf(stdout,"  spit -f dev -c rs0P2000X2     # execute the 2,000 IO operations, twice\n");
  fprintf(stdout,"  spit ..-t 60 -c ws1zM2P1000=  # = alternates r/w per pass. Write 1000, then read 1000, continue for 60s\n");

  fprintf(stdout,"\nSequential/random/striping:\n");
  fprintf(stdout,"  spit -f dev -c s1z            # sequential, one linear region, starting from zero\n");
  fprintf(stdout,"  spit -f dev -c s1             # sequential, one linear region, randomly offset\n");
  fprintf(stdout,"  spit -f dev -c s0             # Not sequential, i.e. random\n");
  fprintf(stdout,"  spit -f dev -c s2             # Two sequential regions. Divided the thread/region into two\n");
  fprintf(stdout,"  spit -f dev -c s9             # Nine sequential regions. Divided the thread/region into nine...\n");
  fprintf(stdout,"  spit -f dev -c s-1z           # Reverse sequential starting from the end of the array\n");
  fprintf(stdout,"  spit -f dev -c s-1            # Reverse sequential, randomly offset\n");
  fprintf(stdout,"  spit -f dev -c s-2            # Two sequential regions, reversed, randomly offset\n");

  fprintf(stdout,"\nBetween linear and random:\n");
  fprintf(stdout,"  spit ... s-1                  # monotonically decreasing\n");
  fprintf(stdout,"  spit ... s0                   # random\n");
  fprintf(stdout,"  spit ... s0.05                # Almost entirely random with a few monotonically increasing positions\n");
  fprintf(stdout,"  spit ... s0.95-20             # With a probability of 0.05 swap with a position +/- 20 positions\n");
  fprintf(stdout,"  spit ... s0.98                # With a probability of 0.02 swap with any other position\n");
  fprintf(stdout,"  spit ... s1                   # monotonically increasing\n");
  
  fprintf(stdout,"\nPositions/latencies:\n");
  fprintf(stdout,"  spit -P filename              # dump positions to filename\n");
  fprintf(stdout,"                                # file contains device, position, size, timing, latency per sample, median latency\n");
  fprintf(stdout,"  spit -P -                     # dump positions to (stdout) and stream raw IOs without collapsing\n");
  fprintf(stdout,"                                # column 2 (byte offset), 6 (operation), 7 (size)\n");
  fprintf(stdout,"                                # column 12/13 (start/fin time), column 14 (mean latency)\n");
  fprintf(stdout,"                                # column 15 (#samples), 16 (median latency)\n");
  fprintf(stdout,"   ... -S positions             # read all the positions from 3 columns. position action and size\n");

  fprintf(stdout,"\nLBA coverage: (using lowercase x)\n");
  fprintf(stdout,"  spit -c P10x1                 # write 10 positions until the entire device size is written\n");
  fprintf(stdout,"  spit -c P10x3                 # write 10 positions until the entire device size is written three times\n");
  fprintf(stdout,"  spit -c x5                    # writing the block device size 5 times, not time based\n");

  fprintf(stdout,"\nPosition coverage: (using capital X)\n");
  fprintf(stdout,"  spit -f ... -c P10X100        # multiply the number of positions by X, here it's 100, so 1,000 positions\n");
  fprintf(stdout,"  spit -f .... -c wns0X10       # writing the number of positions 10 times, not time based\n");
  fprintf(stdout,"  spit -c P10X1                 # write 10 positions\n");
  fprintf(stdout,"  spit -c P10000X100            # write the same 10,000 positions 100 times\n");

  
  fprintf(stdout,"\nComplete coverage: (positions required, size of `LBAbytes / (RAMbytes / 64)`)\n");
  fprintf(stdout,"  spit -f ... -c ..C            # the 'C' command will check the complete LBA is covered, or exit(1)\n");

  fprintf(stdout,"\nDisplays: (-s 1 default, once per second)\n");
  fprintf(stdout,"  spit -s 0.1 -i 5              # display every 0.1 seconds, ignore first 5 GB\n");

  fprintf(stdout,"\nQueue depth: (IO in flight)\n");
  fprintf(stdout,"  spit -f .... -c q128          # per job max queue depth (max inflight is 128, submit one at a time)\n");
  fprintf(stdout,"  spit -f .... -c q32-128       # set the min queue depth to 32, submit at most 32 IOs at a time, max 128\n");
  fprintf(stdout,"  spit -f .... -c q128-128      # set queue depth to fixed at 128 IOs in flight at all time. Uses less CPU.\n");

  fprintf(stdout,"\nSpeed targets: (IOPS target)\n");
  fprintf(stdout,"  spit ... -c ws1S1000q1        # Target 1000 IOPS, with QD=1\n");
  fprintf(stdout,"  spit -c ws1zk10001J2S1000     # Writing monotonically, but from alternating between two threads at 1 MB x 1000/s = 1 GB/s\n");
  fprintf(stdout,"  spit -F. -c ws1zx1j64S100q1 -G1 # creates files from .0001 to .0128, with IOPS targets\n");
  fprintf(stdout,"  spit ... -c ws1S100           # Targets slower IOPS, S100 targets 100 IOPS per thread, with default qd\n");

  fprintf(stdout,"\nWriting/pausing:\n");
  fprintf(stdout,"  spit -f device -c W5          # do 5 seconds worth of IO then wait for 5 seconds\n");
  fprintf(stdout,"  spit -f device -c W0.1:4      # do 0.1 seconds worth of IO then wait for 4 seconds\n");
  fprintf(stdout,"  spit -f ... -c w -cW4rs0      # one thread seq write, one thread, run 4, wait 4 repeat\n");
  fprintf(stdout,"  spit -f ... -c ws1W2:1 -t60   # Alternate run for 2 seconds, wait for 1 second\n");
  fprintf(stdout,"  spit -f .. -c ws0S1           # Write a 4KiB block, randomly, 1 IO per second. The *slow* Loris\n");

  fprintf(stdout,"\nNUMA control:\n");
  fprintf(stdout,"  spit -f -c ..j32 -u           # j32, but do not pin the threads to specific NUMA nodes\n");
  fprintf(stdout,"  spit -f -c ..j32 -U 0         # j32, pin all threads to  NUMA node 0\n");
  fprintf(stdout,"  spit -f -c ..j32 -U 0,1       # j32, split threads evenly between NUMA node 0 and 1\n");
  fprintf(stdout,"  spit -f -c ..j32 -U 0-2       # j32, split threads evenly between NUMA node 0, 1 and 2\n");
  fprintf(stdout,"  spit -f -c ..j32 -U 0,0,1     # j32, allocate threads using a 2:1 ratio between NUMA node 0 and 1\n");

  fprintf(stdout,"\nSee/dump the actions/positions:\n");
  fprintf(stdout,"  spit -f ... -c ... -d 10      # dump the first 10 positions/actions\n");

  fprintf(stdout,"\nLBA ranges/partitioning:\n");
  fprintf(stdout,"  spit -f device -c r -G 1      # 0..1 GB device size\n");
  fprintf(stdout,"  spit -f device -c r -G 384MiB # -G without a range is GB. {M,G,T}[i*]B\n");
  fprintf(stdout,"  spit -f device -c r -G 100GB  # Support GB/GiB etc");
  fprintf(stdout,"  spit -f device -c r -G 1-2    # Only perform actions in the 1-2 GB range\n");
  fprintf(stdout,"  spit -f device -c r -E -G 2-5 # if the -E argument is *before* -G, G values are percentages. e.g. 2%%-5%%\n");
  fprintf(stdout,"  spit -f device -c r -G 2-5 -E # NOTE: This doesn't work. -E must be before -G\n");
  fprintf(stdout,"  spit -c wx3 -G4 -T            # perform pre-DISCARD/TRIM operations before each round\n");
  fprintf(stdout,"  spit -c wx1G0-64k4zs1         # Write from [0,64) GiB, in 4KiB steps, sequentially \n");
  fprintf(stdout,"  spit -c wx1G0-64k4zs1K20      # Write from [0,64) GiB, in 4KiB steps, writing 1 in 20. \n");
  fprintf(stdout,"  spit -c wG_j4                 # The _ represents to divide the G value evenly between threads\n");
  fprintf(stdout,"  spit -c ws1G1-2 -c rs0G2-3    # Seq w in the 1-2 GB region, rand r in the 2-3 GB region\n");
  fprintf(stdout,"  spit -f ... -c P10G1-2        # The first 10 positions starting from 1GiB. It needs the lower range.\n");
  fprintf(stdout,"  spit -c ws1G5_10j16           # specify a low and high GiB range, to be evenly split by 16 threads (_)\n");

  
  fprintf(stdout,"\nLogging for experiments:\n");
  fprintf(stdout,"  spit -f ... -l logfile        # will append the run results to 'logfile'. Works well with ./combo expansion\n");
  fprintf(stdout,"  spit -B bench -M ... -N ...   # See the man page for benchmarking tips\n");

  fprintf(stdout,"\nBlock sizes:\n");
  fprintf(stdout,"  spit -f device -c k8          # set block size to 8 KiB\n");
  fprintf(stdout,"  spit -f device -c k4-128      # set block range to 4 to 128 KiB, every 4 KiB\n");
  fprintf(stdout,"  spit -f device -c k4:1024     # set block range to 4 to 1024 KiB, in powers of 2\n");
  fprintf(stdout,"  spit -f ... -c wM1            # set block size 1M\n");

  fprintf(stdout,"\nRange resets:\n");
  fprintf(stdout,"  spit -f device -c s1k8A1        # set block size to 8 KiB, randomize next pos after 1 MiB of data\n");
  fprintf(stdout,"  spit -f device -c s1k4-16A0.5-8 # set block size to 4-16 KiB, randomize after 0.5 to 8 MiB\n");
  fprintf(stdout,"  spit -f device -c s1k4:16A0.5:8 # As above but power of 2 ranges\n");

  fprintf(stdout,"\nI/O amplification: (-O underlying_devices.txt)\n");
  fprintf(stdout,"  spit -f dev -O devices.txt    # specify the raw devices for amplification statistics\n");
  fprintf(stdout,"  spit -f dev -O <(echo dev)    # use BASH syntax for creation of a virtual inline fd\n");
  fprintf(stdout,"  spit .. -c rs1k4q1 -O ..      # random 4 KiB, times each block read when contiguous\n");
  fprintf(stdout,"  spit .. -c zrs1k4q1j128 -O .. # read the same ranges from 128 threads, are they cached\n");
  fprintf(stdout,"  spit .. -c rs0k4q1 -O ..      # random 4 KiB, how much extra is read\n");
  fprintf(stdout,"  spit .. -c rs1M2q1 -O ..      # sequential random 2 MiB, check amp\n");


  fprintf(stdout,"\nRead-ahead (testing seq reads with qd=1)\n");
  fprintf(stdout,"  spit .. -c rs1k4q1 -O ..       # sequential 4 KiB reads, check amplification\n");
  fprintf(stdout,"  spit .. -c rs1k4-128q1 -O ..   # sequential 4-128 KiB reads, check amplification\n");
  fprintf(stdout,"  spit .. -c rs1k64A4q1 -O ..    # 64 KiB reads, reset position every 4 MiB ('A')\n");
  fprintf(stdout,"  spit .. -c rs1k64K4q1 -O ..    # 64 KiB reads, only doing every 4th action ('K')\n");
  fprintf(stdout,"  spit .. -c rs1M2q1 -O ..       # seq 2 MiB reads\n");
  fprintf(stdout,"  spit .. -c rs-1M2q1 -O ..      # reverse seq/backwards 2 MiB reads (!)\n");


  fprintf(stdout,"\nI/O write verification: (-v to verify)\n");
  fprintf(stdout,"  spit -v                       # verify the writes after a run\n");
  fprintf(stdout,"  spit ... -c ws0u -v           # Uses a unique seed (u) per operation (mod 65536)\n");
  fprintf(stdout,"  spit ... -c ws0U -v           # Generates a read immediately after a write (U), tests with qd=1\n");
  fprintf(stdout,"  spit ... -c ws0UG_j32 -v      # Generates r/w pairs with unique seeds, as above, unique thread ranges\n");

  fprintf(stdout,"\nPreconditioning:\n");
  fprintf(stdout,"  spit -p G100s1k64             # precondition job, sequential, 64 KiB blocks\n");
  fprintf(stdout,"  spit -p G -p Gs1              # precondition job, writing random, 100%% LBA, then seq job\n");
  fprintf(stdout,"  spit -p G100                  # precondition job, writing random overwrite LBA size\n");
  fprintf(stdout,"  spit -p f5 -f device -c ...   # Precondition/max-fragmentation with 5%% GC overhead, becomes K20.\n");
  fprintf(stdout,"                                # the -p f5 commands iterates across the LBA in 64G chunks as previous two commands.\n");

  fprintf(stdout,"\nO_DIRECT vs pagecache: (default O_DIRECT)\n");
  fprintf(stdout,"  spit -f ... -c rD0            # 'D' turns off O_DIRECT\n");

  fprintf(stdout,"\nMisc examples:\n");
  fprintf(stdout,"  spit -f device -c ... -c ... -c ... # defaults to %d seconds\n", DEFAULTTIME);
  fprintf(stdout,"  spit -f device -c r           # seq read (defaults to s1 and k4)\n");
  fprintf(stdout,"  spit -f device -c w           # seq write (s1)\n");
  fprintf(stdout,"  spit -f device -c w -L1       # Limit RAM use to 1 GiB. 48 bytes is used per position\n");
  fprintf(stdout,"  spit -f device -c w           # Default RAM allocated to positions is MIN(15GB, free RAM/2)\n");
  fprintf(stdout,"  spit -f device -c rs0         # random, (s)equential is 1\n");
  fprintf(stdout,"  spit -f device -c rs1         # sequential\n");
  fprintf(stdout,"  spit -f device -c rzs1        # sequential starting from position 0\n");
  fprintf(stdout,"  spit -f device -c rzs-1       # reverse sequential starting from end of the device\n");
  fprintf(stdout,"  spit -f device -c rs0-size    # random, (s)equential is 0, max contig is size KiB\n");
  fprintf(stdout,"  spit -f device -c ws2         # 2 contiguous regions on the device. Add -d 10 to dump positions\n");
  fprintf(stdout,"  spit -f device -c ws128       # 128 contiguous regions on the device\n");
  fprintf(stdout,"  spit -f device -c \"r s128 k4\" -c \'w s4 -k128\' -c rw\n");
  fprintf(stdout,"  spit -f device -b 10240000    # specify the max device size in bytes\n");
  fprintf(stdout,"  spit -f ... -t 50             # run for 50 seconds (-t -1 is forever)\n");
  fprintf(stdout,"  spit -f -c ..j32              # duplicate all the commands 32 times. Pin threads to each NUMA node.\n");
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
  fprintf(stdout,"  spit -f ... -c O              # One-shot, not time based\n");
  fprintf(stdout,"  spit -f ... -c t2             # specify the time per thread\n");
  fprintf(stdout,"  spit -f ... -c ws1F4          # jumble/reverse groups of 4 positions\n");
  fprintf(stdout,"  spit -f ... -c wx20           # sequentially (s1) write 20x LBA, no time limit\n");
  fprintf(stdout,"  spit -f ... -c ws0            # random defaults to 3x LBA\n");
  fprintf(stdout,"  spit -I devices.txt -c r      # -I is read devices from a file\n");
  fprintf(stdout,"  spit -f .... -R seed          # set the initial seed, j will increment per job\n");
  fprintf(stdout,"  spit -c P10000                # write the same 10,000 positions based on the time\n");
  fprintf(stdout,"  spit -c P-10000               # A -ve P number, is determine 10,000 positions with replacement. Don't verify\n");
  fprintf(stdout,"  spit -c wZ1                   # Z is the starting offset. -z is -Z0\n");
  fprintf(stdout,"  spit -F fileprefix -c ..j128  # creates files from .0001 to .0128\n");
  fprintf(stdout,"  spit -e \"5,echo five\"         # exec a bash -c CMD string after 5 seconds, quotes are required\n");
  fprintf(stdout,"  spit -c wk1024za7             # every 'a' MiB of operations perform a jump back to the start of device. Dump with -d to see\n");
  fprintf(stdout,"  spit -c ts0                   # Use a sync DISCARD/TRIM I/O type\n");
  fprintf(stdout,"  spit -c rrwts0                # 50%% read, 25%% writes and 25%% trim I/O random operations\n");
  fprintf(stdout,"  spit ... -c rs0Y1000          # Limit the number of positions to process to 1,000 then end the run\n");
  fprintf(stdout,"  spit -c ... -D                # Display the 'D'ate/time per line\n");
  fprintf(stdout,"  spit -f .. -c ws0J8           # Use 8 threads, but only do every 8th IO in each thread. For NUMA/S testing\n");
  
  exit(0);
}


void intHandler(int d)
{
  if (d) {}
  fprintf(stderr,"got signal\n");
  keepRunning--;
}


int doReport(const double runseconds, size_t maxSizeInBytes, const size_t cacheSizeBytes, const size_t forever,
	     const size_t verify, size_t ramBytesForPositions, size_t defaultqd, size_t showdate)
{

  if (device == NULL) {
    fprintf(stderr,"*error* no -f device provided\n");
    return 1;
  }

  if (!canOpenExclusively(device)) {
    perror(device);
    fprintf(stderr,"*error* device already open!\n");
    return 1;
  }

  if (maxSizeInBytes == 0) maxSizeInBytes = 50 * 1024 * 1024;

  char text[100];
  time_t now = time(NULL);
  struct tm *t = localtime(&now);

  strftime(text, sizeof(text)-1, "%Y-%m-%d", t);


  fprintf(stdout, "== Report: `stutools: spit`\n");

  char *host = hostname();
  fprintf(stdout, " Machine: %s\n", host);
  fprintf(stdout, " Report date: %s\n", text);

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
  fprintf(stdout, " Cache size: %.3lf GB\n", TOGiB(cacheSizeBytes));
  fprintf(stdout, " Testing time: %.1lf seconds\n", runseconds);

  size_t discard_max_bytes, discard_granularity, discard_zeroes_data, alignment_offset;
  getDiscardInfo(suffix, &alignment_offset, &discard_max_bytes, &discard_granularity, &discard_zeroes_data);
  fprintf(stdout, " Trim/Discard: alignment_offset %zd, max_bytes %zd (%.1lf MiB), granularity %zd, zeroes_data %zd\n", alignment_offset, discard_max_bytes, TOGiB(discard_max_bytes), discard_granularity, discard_zeroes_data);
  if (suffix) free(suffix);
  suffix = NULL;


  char *cpumodel = getCPUModel();
  fprintf(stdout, " CPU: %s\n", cpumodel);
  if (cpumodel) free(cpumodel);
  if (host) free(host);
  if (defaultqd < 64) defaultqd = 64;
  fprintf(stdout, " Queue depth total: %zd\n", defaultqd);
  fprintf(stdout, " NUMA nodes: %d\n", getNumaCount());
  fprintf(stdout, " Thread count: %d\n", getNumHardwareThreads());
  char *os = OSRelease();
  fprintf(stdout, " OS: %s\n", os);
  if (os) free(os);
  fprintf(stdout, " RAM: %.0lf GiB (%.1lf GiB)\n", TOGiB(totalRAM()), TOGiB(ramBytesForPositions));
  fprintf(stdout, " stutools: %s\n", VERSION);
  fprintf(stdout, "\n\n");

  diskStatType d;
  diskStatSetup(&d);

  size_t fsize = MIN(maxSizeInBytes, fileSizeFromName(device)); // the first xx MiB

  if (fsize <= 0) {
    fprintf(stderr, "*error* no file called '%s'\n", device);
    return 1;
  }

  //  size_t blockSize1[] = {4,4,8,16,8, 32,64,128,256,128,512,1024,2048,4, 4096};
  //  size_t blockSize2[] = {4,8,8,16,64,32,64,128,256,512,512,1024,2048,4096, 4096};
  size_t blockSize1[] = {8*1024,4 * 1024, 2*1024, 1024, 512, 256, 64, 16, 8, 4};
  size_t blockSize2[] = {8*1024,4 * 1024, 2*1024, 1024, 512, 256, 64, 16, 8, 4};

  size_t threadBlock[] = {1,64};

  size_t dedupSizes[] = {1,10,100,1000,10000,100000,1000000};
  char s[100];
  jobType j;

  double starttime = timedouble();
  int round = 0;
  resultType r;  memset(&r, 0, sizeof(resultType));
  
  size_t xcopies = 1;
  double thetime = runseconds;

  if (forever) {
    fprintf(stderr,"*info* running forever...\n");
  }

  while (forever || (timedouble() - starttime < runseconds)) {
      char ss[200];
      now = time(NULL);
      t = localtime(&now);
      strftime(text, sizeof(text)-1, "%Y-%m-%d %H:%M:%S", t);

      sprintf(ss, "=== Round %d (%s, elapsed %.1lf seconds, %.2lf days)\n", ++round, text, timedouble() - starttime, (timedouble() - starttime) / 86400.0);
      fprintf(stdout, "%s\n", ss);
      syslogString("spit", ss);


      fprintf(stdout, "==== Random Write\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read MB/s | Write MB/s \n");

      if (1) for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
          for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
            jobInit(&j);
            size_t qd = defaultqd;
            for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {

              qd = defaultqd / threadBlock[t];
              if (qd < 1) qd = 1;
	      if (qd > 64) qd = 64;

              sprintf(s, "w s0 k%zd-%zd G_ j%zd#%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], t2, qd, thetime * 2);
              jobAdd(&j, s);
            }
            jobAddDeviceToAll(&j, device);
            sprintf(s, "w s0 k%zd-%zd G_ j%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], qd, thetime * 2);
            fprintf(stdout, "| %s ", s);
            fflush(stdout);
            jobRunThreads(&j, j.count, NULL, 0, bdsize, thetime * 2, 0, NULL, 4, round+42, 0, NULL /* diskstats &d*/, MIN(thetime/10.0, 1), cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0,  &r, ramBytesForPositions, 0, showdate, NULL);
            fprintf(stdout, "| %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf \n", threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
            fflush(stdout);
          }
        }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);


      fprintf(stdout, "==== Sequential Write\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read MB/s | Write MB/s\n");

      if(1) for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
          for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
            jobInit(&j);
            size_t qd = defaultqd;
            for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
              qd = defaultqd /  threadBlock[t];
              if (qd < 1) qd = 1;
	      if (qd > 64) qd = 64;

              sprintf(s, "w s1 k%zd-%zd G_ j%zd#%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], t2, qd, thetime * 2);
              jobAdd(&j, s);
            }
            jobAddDeviceToAll(&j, device);
            jobRunThreads(&j, j.count, NULL, 0, bdsize, thetime * 2, 0, NULL, 4, round+42, 0, NULL /* diskstats &d*/, MIN(thetime/10.0, 1), cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0,  &r, ramBytesForPositions, 0, showdate, NULL);
            sprintf(s, "w s1 k%zd-%zd G_ j%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], qd, thetime * 2);
            fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
            fflush(stdout);
          }
        }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);



      fprintf(stdout, "==== Random Read\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read MB/s | Write MB/s\n");

      for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
        for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
          jobInit(&j);
          size_t qd = defaultqd;
          for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
            qd = defaultqd /  threadBlock[t];
              if (qd < 1) qd = 1;
	      if (qd > 64) qd = 64;
            sprintf(s, "r s0 k%zd-%zd G_ j%zd#%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], t2, qd, thetime);
            jobAdd(&j, s);
          }
          jobAddDeviceToAll(&j, device);
          jobRunThreads(&j, j.count, NULL, 0, bdsize, thetime, 0, NULL, 16, round+42, 0, NULL /* diskstats &d*/, MIN(thetime/10.0, 1), cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0,  &r, ramBytesForPositions, 0, showdate, NULL);
          sprintf(s, "r s0 k%zd-%zd G_ j%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], qd, thetime);
          fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
          fflush(stdout);
        }
      }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);



      fprintf(stdout, "==== Sequential Read\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read MB/s | Write MB/s\n");

      for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
        for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
          jobInit(&j);
          size_t qd = defaultqd;
          for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
            qd = defaultqd /  threadBlock[t];
              if (qd < 1) qd = 1;
	      if (qd > 64) qd = 64;
            sprintf(s, "r s1 k%zd-%zd G_ j%zd#%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], t2, qd, thetime);
            jobAdd(&j, s);
          }
          jobAddDeviceToAll(&j, device);
          jobRunThreads(&j, j.count, NULL, 0, bdsize, thetime, 0, NULL, 16, round+42, 0, NULL /* diskstats &d*/, MIN(thetime/10.0, 1), cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0,  &r, ramBytesForPositions, 0, showdate, NULL);
          sprintf(s, "r s1 k%zd-%zd G_ j%zd q1-%zd T%.1lf", blockSize1[i], blockSize2[i], threadBlock[t], qd, thetime);
          fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
          fflush(stdout);
        }
      }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);


      fprintf(stdout, "==== Write de-dup\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command | Threads |  Read IOPS | Write IOPS | Read MB/s | Write MB/s\n");

      for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
        for (size_t i = 0 ; i < sizeof(dedupSizes) / sizeof(size_t); i++) {
          jobInit(&j);
          for (size_t jj = 0 ; jj < threadBlock[t]; jj++) {
            sprintf(s, "w s0 P%zd G_ j%zd#%zd q16 G_ k4 T%.1lf", dedupSizes[i], threadBlock[t], jj, thetime);
            jobAdd(&j, s);
          }
          jobAddDeviceToAll(&j, device);
          sprintf(s, "w s0 P%zd G_ j%zd q16 G_ k4 T%.1lf", dedupSizes[i], threadBlock[t], thetime);
          size_t low = 0, high = fsize;
          if (fsize * (i+1) <= bdsize) {
            low = fsize * i;
            high = fsize * (i+1);
          }
          jobRunThreads(&j, j.count, NULL, low, high, -1, 0, NULL, 4, round+42, 0, NULL /* diskstats &d*/, MIN(thetime/10.0, 1), cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0,  &r, ramBytesForPositions, 0, showdate, NULL);
          fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
          fflush(stdout);
        }
      }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);


      fprintf(stdout, "==== Random Write metadata + read\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command |  Threads | Read IOPS | Write IOPS | Read MB/s | Write MB/s\n");

      for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
        for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
          jobInit(&j);
          size_t qd = defaultqd;
          for (size_t jj = 0 ; jj < threadBlock[t]; jj++) {
            qd = defaultqd /  threadBlock[t];
              if (qd < 1) qd = 1;
	      if (qd > 64) qd = 64;
            sprintf(s, "w s0 k%zd P100000 q1-%zd G_ j%zd#%zd T%.1lf", blockSize1[i], qd, threadBlock[t], jj, thetime);
            jobAdd(&j, s);
            sprintf(s, "r s0 k%zd P100000 q1-%zd G_ j%zd#%zd T%.1lf", blockSize2[i], qd, threadBlock[t], jj, thetime);
            jobAdd(&j, s);
          }

          jobAddDeviceToAll(&j, device);
          jobRunThreads(&j, j.count, NULL, 0, bdsize, thetime, 0, NULL, 32, round+42, NULL /* save positions*/, NULL /* diskstats &d*/, 0.1 /*timeline*/, cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0, &r, ramBytesForPositions, 0, showdate, NULL);
          sprintf(s, "rs0k%zdP100000q1-%zdG_j%zd/+wk%zd", blockSize1[i], qd, threadBlock[t], blockSize2[i]);
          fprintf(stdout, "| %s | %zd |  %.0lf | %.0lf |  %.1lf |  %.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
          fflush(stdout);
        }
      }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);


      //
      fprintf(stdout, "==== Random 70%% Read / 30%% Write\n\n");
      fprintf(stdout, "[cols=\"<4,^1,^1,^1,^1,^1\", options=\"header\"]\n");
      fprintf(stdout, "|===\n");
      fprintf(stdout, "| Command | Threads | Read IOPS | Write IOPS | Read MB/s | Write MB/s\n");


      for (size_t t = 0; t < sizeof(threadBlock) / sizeof(size_t); t++) {
        for (size_t i = 0 ; i < sizeof(blockSize1) / sizeof(size_t); i++) {
          jobInit(&j);
          size_t qd = defaultqd;
          for (size_t t2 = 0; t2 < threadBlock[t]; t2++) {
            qd = defaultqd /  threadBlock[t];
	    if (qd < 1) qd = 1;
	    if (qd > 64) qd = 64;
            sprintf(s, "p0.7 s0 k%zd-%zd q1-%zd G_ j%zd#%zd T%.1lf", blockSize1[i], blockSize2[i], qd, threadBlock[t], t2, thetime);
            jobAdd(&j, s);
          }
          jobAddDeviceToAll(&j, device);
          jobRunThreads(&j, j.count, NULL, 0, bdsize, thetime, 0, NULL, 32, round+42, NULL /* save positions*/, NULL /* diskstats &d*/, 0.1 /*timeline*/, cacheSizeBytes, verify, NULL, NULL, NULL, "all", 0, &r, ramBytesForPositions, 0, showdate, NULL);
          sprintf(s, "p0.7 s0 k%zd-%zd q1-%zd G_ j%zd x%zd", blockSize1[i], blockSize2[i], qd, threadBlock[t], xcopies);
          fprintf(stdout, "| %s | %zd  |   %.0lf | %.0lf |  %.1lf |  %.1lf\n", s, threadBlock[t], r.readIOPS, r.writeIOPS, TOMB(r.readBps), TOMB(r.writeBps));
          fflush(stdout);
        }
      }
      fprintf(stdout, "|===\n\n");
      fflush(stdout);

    } // while

  diskStatFree(&d);

  return 0;
}


/**
 * main
 *
 */
int main(int argc, char *argv[])
{
  sysLogArgs("spit", argc, argv);

  size_t fuzz = 0, runcount = 0;
  char *fuzzdevice = NULL;
  int exitcode = 0;
  
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

    size_t defaultQD = 0;
    unsigned short seed = 0;
    diskStatType d;
    size_t verify = 0;
    double timeperline = 1, ignoreFirst = 0;
    char *numaBinding = "all";
    int performPreDiscard = 0;
    int reportMode = 0;

    diskStatSetup(&d);
    double runseconds = DEFAULTTIME;
    size_t minSizeInBytes = 0, maxSizeInBytes = 0, dumpPositions = 0;
    char *mysqloptions = NULL, *mysqloptions2 = NULL;
    size_t cacheSizeBytes = 0, forever = 0;
    size_t ramBytesForPositions = 0, tripleX = 0;
    size_t showdate = 0;

    char *kcheckresults = NULL;
    char commandstring[1000];
    FILE *loadpos = NULL;

    handle_args(argc2, argv2, preconditions, j, &minSizeInBytes, &maxSizeInBytes, &runseconds, &dumpPositions, &seed, &d, &verify, &timeperline, &ignoreFirst, &mysqloptions, &mysqloptions2, commandstring, &filePrefix, &numaBinding, &performPreDiscard, &reportMode, &cacheSizeBytes, &forever, &ramBytesForPositions, &tripleX, &showdate, &kcheckresults, &loadpos);

    if (runseconds <= 1 && runseconds > 0 && timeperline == 1) {
      timeperline = runseconds / 10;
    }

    if (reportMode) {
      if (defaultQD == 0) defaultQD = 1000; // for a report -q is the total across all threads.
      exitcode = doReport(runseconds, maxSizeInBytes, cacheSizeBytes, forever, verify, ramBytesForPositions, defaultQD, showdate);
    } else if (j->count < 1) {
      fprintf(stderr,"*error* missing -c command options\n");
      exitcode = 2;
    } else { // run some jobs
      if (defaultQD == 0) defaultQD = 64;

      printPowerMode();

      size_t actualSize = maxSizeInBytes - minSizeInBytes;
      fprintf(stderr,"*info* block range [%.2lf-%.2lf] GiB, [%.2lf-%.2lf] GB, size %.2lf GiB (%zd bytes). Range [%.3lf-%.3lf] TB\n", TOGiB(minSizeInBytes), TOGiB(maxSizeInBytes), TOGB(minSizeInBytes), TOGB(maxSizeInBytes), TOGiB(actualSize), actualSize, TOTB(minSizeInBytes), TOTB(maxSizeInBytes));
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

      resultType r; memset(&r, 0, sizeof(resultType));

      jobRunThreads(j, j->count, filePrefix, minSizeInBytes, maxSizeInBytes, runseconds, dumpPositions, benchmarkName, defaultQD, seed, savePositions, p, timeperline, ignoreFirst, verify, mysqloptions, mysqloptions2, commandstring, numaBinding, performPreDiscard, &r, ramBytesForPositions, tripleX==3, showdate, loadpos);

      resultDump(&r, kcheckresults, verbose);

      if (logfile) resultLog(logfile, &r, lowg, highg, j);

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
  if (device) free(device);

  fprintf(stderr,"*info* exiting.\n");
  fflush(stderr);
  exit(exitcode);
}


