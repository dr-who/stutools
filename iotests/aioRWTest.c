#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/types.h>
#include <syslog.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>
#include <assert.h>

#include "aioRequests.h"
#include "utils.h"
#include "logSpeed.h"
#include "diskStats.h"
#include "positions.h"

int    keepRunning = 1;       // have we been interrupted
double exitAfterSeconds = 60;
int    qd = 256;
int    qdSpecified = 0;
char   **pathArray = NULL;
size_t pathLen = 0;
int    seqFiles = 1;
int    seqFilesSpecified = 0;
double maxSizeGB = 0;
size_t alignment = 0;
size_t LOWBLKSIZE = 65536;
size_t BLKSIZE = 65536;
int    jumpStep = 1;
double readRatio = 0.5;
size_t table = 0;
char   *logFNPrefix = NULL;
int    verbose = 0;
int    singlePosition = 0;
int    flushEvery = 0;
size_t noops = 1;
int    verifyWrites = 0;
char*  specifiedDevices = NULL;
int    sendTrim = 0;
int    startAtZero = 0;
size_t maxPositions = 10*1000*1000;
size_t dontUseExclusive = 0;
char*  logPositions = NULL;
long int seed;

void addDeviceToAnalyse(const char *fn) {
  pathArray = (char**)realloc(pathArray, (pathLen + 1) * sizeof(char *));
  pathArray[pathLen] = strdup(fn);
  if (verbose >= 2) {
    fprintf(stderr,"*info* adding -f '%s'\n", pathArray[pathLen]);
  }
  pathLen++;
}

size_t loadSpecifiedFiles(const char *fn) {
  size_t add = 0;
  FILE *fp = fopen(fn, "rt");
  if (!fp) {
    perror(fn);
    return add;
  }

  char *line = NULL;
  size_t len = 0;
  ssize_t read;
  
  while ((read = getline(&line, &len, fp)) != -1) {
    //    printf("Retrieved line of length %zu :\n", read);
    line[strlen(line)-1] = 0; // erase the \n
    addDeviceToAnalyse(line);
    add++;
    //    printf("%s", line);
  }
  
  free(line);

  fclose(fp);
  return add;
}


void handle_args(int argc, char *argv[]) {
  int opt;
  seed = (long int) timedouble();
  if (seed < 0) seed=-seed;
  
  while ((opt = getopt(argc, argv, "dDt:k:o:q:f:s:G:j:p:Tl:vVSF0R:O:rwb:MgzP:Xa:L:I:")) != -1) {
    switch (opt) {
    case 'a':
      alignment = atoi(optarg) * 1024;
      if (alignment < 1024) alignment = 1024;
      break;
    case 'L':
      logPositions = strdup(optarg);
      break;
    case 'I':
      {}
      size_t added = loadSpecifiedFiles(optarg);
      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
      //      inputFilenames = strdup(optarg);
      break;
    case 'X':
      dontUseExclusive++;
      break;
    case 'P':
      maxPositions = atoi(optarg);
      if (verbose) {
	fprintf(stderr,"*info* hard coded maximum number of positions %zd\n", maxPositions);
      }
      break;
    case 'z':
      startAtZero = 1;
      break;
    case 'M':
      sendTrim = 1;
      break;
    case 'T':
      table = 1;
      break;
    case 'O':
      if (!specifiedDevices) {
	specifiedDevices = strdup(optarg);
      }
      break;
    case '0':
      noops = 0;
      break;
    case 'r':
      readRatio += 0.5;
      if (readRatio > 1) readRatio = 1;
      break;
    case 'w':
      readRatio -= 0.5;
      if (readRatio < 0) readRatio = 0;
      break;
    case 'R':
      seed = atoi(optarg);
      break;
    case 'S':
      if (singlePosition == 0) {
	singlePosition = 1;
      } else {
	singlePosition = 10 * singlePosition;
      }
      break;
    case 'F':
      if (flushEvery == 0) {
	flushEvery = 1;
      } else {
	flushEvery = 10 * flushEvery;
      }
      break;
    case 'v':
      verifyWrites = 1;
      break;
    case 'V':
      verbose++;
      break;
    case 'l':
      logFNPrefix = strdup(optarg);
      break;
    case 't':
      exitAfterSeconds = atof(optarg); 
      break;
    case 'q':
      qd = atoi(optarg); if (qd < 1) qd = 1;
      qdSpecified = 1;
      break;
    case 's':
      seqFiles = atoi(optarg);
      seqFilesSpecified = 1;
      break;
    case 'b':
      seqFiles = -atoi(optarg);
      fprintf(stderr,"*info* backwards contiguous: %d\n", seqFiles);
      break;
    case 'j':
      jumpStep = atoi(optarg); 
      break;
    case 'G':
      maxSizeGB = atof(optarg);
      break;
    case 'k': {
      char *ndx = index(optarg, '-');
      if (ndx) {
	int firstnum = atoi(optarg) * 1024;
	int secondnum = atoi(ndx + 1) * 1024;
	if (secondnum < firstnum) secondnum = firstnum;
	if (verbose > 1) {
	  fprintf(stderr,"*info* specific block range: %d KiB (%d) to %d KiB (%d)\n", firstnum/1024, firstnum, secondnum/1024, secondnum);
	}
	LOWBLKSIZE = firstnum;
	BLKSIZE = secondnum;
	// range
      } else {
	BLKSIZE = 1024 * atoi(optarg); if (BLKSIZE < 1024) BLKSIZE=1024;
	LOWBLKSIZE = BLKSIZE;
      }}
      break;
    case 'p':
      readRatio = atof(optarg);
      if (readRatio < 0) readRatio = 0;
      if (readRatio > 1) readRatio = 1;
      break;
    case 'f':
      addDeviceToAnalyse(optarg);
      /*      pathArray = (char**)realloc(pathArray, (pathLen + 1) * sizeof(char *));
      pathArray[pathLen] = strdup(optarg);
      if (verbose >= 2) {
	fprintf(stderr,"*info* adding -f '%s'\n", pathArray[pathLen]);
      }
      pathLen++;*/
      //      path = optarg;
      break;
    default:
      exit(-1);
    }
  }
  if (pathLen < 1) {
    fprintf(stderr,"./aioRWTest [-s sequentialFiles] [-j jumpBlocks] [-k blocksizeKB] [-q queueDepth] [-t 30 secs] [-G 32] [-p readRatio] -f blockdevice\n");
    fprintf(stderr,"\nExample:\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0          # 50/50 read/write test, seq r/w\n");
    fprintf(stderr,"  ./aioRWTest -I devicelist.txt     # 50/50 read/write test, seq r/w from a file\n");
    fprintf(stderr,"  ./aioRWTest -r -f /dev/nbd0       # read test\n");
    fprintf(stderr,"  ./aioRWTest -w -f /dev/nbd0       # write test\n");
    fprintf(stderr,"  ./aioRWTest -w -V -f /dev/nbd0    # write test, verbosity, including showing the first N positions\n");
    fprintf(stderr,"  ./aioRWTest -r -s1 -I devs.txt    # read test, single contiguous region\n");
    fprintf(stderr,"  ./aioRWTest -w -s128 -f /dev/nbd0 # write test, 128 parallel contiguous regions\n");
    fprintf(stderr,"  ./aioRWTest -S -F -f /dev/nbd0    # single static position, fsync after every op\n");
    fprintf(stderr,"  ./aioRWTest -p0.25 -f /dev/nbd0   # 25%% write, and 75%% read\n");
    fprintf(stderr,"  ./aioRWTest -p1 -f /dev/nbd0 -k4 -q64 -s32 -j16  # 100%% reads over entire block device\n");
    fprintf(stderr,"  ./aioRWTest -p1 -f /dev/nbd0 -k4 -q64 -s32 -j16  # 100%% reads over entire block device\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -G100    # limit actions to first 100GiB\n");
    fprintf(stderr,"  ./aioRWTest -p0.1 -f/dev/nbd0 -G3 # 90%% reads, 10%% writes, limited to first 3GiB of device\n");
    fprintf(stderr,"  ./aioRWTest -t30 -f/dev/nbd0      # test for 30 seconds\n");
    fprintf(stderr,"  ./aioRWTest -S -S -F -F -k4 -f /dev/nbd0  # single position, changing every 10 ops, fsync every 10 ops\n");
    fprintf(stderr,"  ./aioRWTest -0 -F -f /dev/nbd0    # send no operations, then flush. Basically, fast flush loop\n");
    fprintf(stderr,"  ./aioRWTest -S -F -V -f /dev/nbd0 # verbose that shows every operation\n");
    fprintf(stderr,"  ./aioRWTest -S -F -V -f file.txt  # can also use a single file. Note the file will be destroyed.\n");
    fprintf(stderr,"  ./aioRWTest -v -t15 -p0.5 -f /dev/nbd0  # random positions, 50%% R/W, verified after 15 seconds.\n");
    fprintf(stderr,"  ./aioRWTest -v -t15 -p0.5 -R 9812 -f /dev/nbd0  # set the starting seed to 9812\n");
    fprintf(stderr,"  ./aioRWTest -s 1 -j 10 -f /dev/sdc -V   # contiguous access, jumping 10 blocks at a time\n");
    fprintf(stderr,"  ./aioRWTest -s -8 -f /dev/sdc -V  # reverse contiguous 8 regions in parallel\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -O ok    # use a list of devices in ok.txt for disk stats/amplification\n");
    fprintf(stderr,"  ./aioRWTest -M -f /dev/nbd0 -G20  # -M sends trim/discard command, using -G range if specified\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -k1 -G0.1 # write 1KiB buffers from 100 MiB (100,000 unique). Cache testing.\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -P 10000 -f /dev/nbd0 -k1 -G0.1 # Use 10,000 positions. Cache testing.\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -XXX # Triple X does not use O_EXCL. For multiple instances simultaneously.\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -XXX -z # Start the first position at position zero instead of random.\n");
    fprintf(stderr,"  ./aioRWTest -s1 -w -f /dev/nbd0 -P10000 -z -a1 # align operations to 1KiB instead of the default 4KiB\n");
    fprintf(stderr,"  ./aioRWTest -L log.txt -s1 -w -f /dev/nbd0 # log all positions and operations to log.txt\n");
    fprintf(stderr,"\nTable summary:\n");
    fprintf(stderr,"  ./aioRWTest -T -t 2 -f /dev/nbd0  # table of various parameters\n");
    exit(1);
  }

}






size_t testReadLocation(int fd, size_t b, size_t blksize, positionType *positions, size_t num, char *randomBuffer, const size_t randomBufferSize) {
  size_t pospos = ((size_t) b / blksize) * blksize;
  fprintf(stderr,"position %6.2lf GiB: ", TOGiB(pospos));
  positionType *pos = positions;
  size_t posondisk = pospos;
  for (size_t i = 0; i < num; i++) {
    pos->pos = posondisk;  pos->action = 'R'; pos->success = 0; pos->len = BLKSIZE;
    posondisk += (BLKSIZE);
    if (posondisk > b + 512*1024*1024L) {
      posondisk = b;
    }
    pos++;
  }
      
  double start = timedouble();
  size_t ios = 0, totalRB = 0, totalWB = 0;
  size_t br = aioMultiplePositions(positions, num, exitAfterSeconds, qd, verbose, 1, NULL, randomBuffer, randomBufferSize, alignment, &ios, &totalRB, &totalWB);
  double elapsed = timedouble() - start;
  //ios = ios / elapsed;
  fprintf(stderr,"ios %.1lf %.1lf MiB/s\n", ios/elapsed, TOMiB(br/elapsed));

  return br;
}


int similarNumbers(double a, double b) {
  double f = MIN(a, b) * 1.0 / MAX(a, b);
  //  fprintf(stderr,"similar %lf %lf,  f %lf\n", a, b, f);
  if ((f > 0.8) && (f < 1.2)) {
    return 1;
  } else {
    return 0;
  }
}


size_t openArrayPaths(char **p, size_t const len, int *fdArray, size_t *fdLen, const size_t sendTrim, const size_t maxSizeGB) {
  size_t retBD = 0;
  size_t actualBlockDeviceSize = 0 ;
  
  //  int error = 0;
  
  char newpath[4096];
  //  CALLOC(newpath, 4096, sizeof(char));
    
  for (size_t i = 0; i < len; i++) {
    memset(newpath, 0, 4096);
    // follow a symlink
    fdArray[i] = -1;
    realpath(p[i], newpath);

    if (verbose >= 2) {
      fprintf(stderr,"*info* processing path %s\n", newpath);
    }

    size_t phy, log;
    if (isBlockDevice(newpath)) {
      /* -- From a Google search
	 The physical block size is the smallest size the device can
	 handle atomically. The smalles block it can handle without needing
	 to do a read-modify-write cycle. Applications should try to write
	 in multiples of that (and they do) but are not required to.

	 The logical block size on the other hand is the smallest size the
	 device can handle at all. Applications must write in multiples of
	 that.
      */
									
      char *suffix = getSuffix(newpath);
      char *sched = getScheduler(suffix);
      if (verbose >= 2) {
	fprintf(stderr,"  *info* scheduler for %s (%s) is [%s]\n", newpath, suffix, sched);
      }
    
      getPhyLogSizes(suffix, &phy, &log);
      if (verbose >= 2) {
	fprintf(stderr,"  *info* physical_block_size %zd, logical_block_size %zd\n", phy, log);
      }

      if (LOWBLKSIZE < log) {
	fprintf(stderr,"  *warning* the block size is lower than the device logical block size\n");
	LOWBLKSIZE = log;
	if (LOWBLKSIZE > BLKSIZE) BLKSIZE=LOWBLKSIZE;
      }
      
      // various warnings and informational comments
      if (LOWBLKSIZE < alignment) {
	LOWBLKSIZE = alignment;
	if (LOWBLKSIZE > BLKSIZE) BLKSIZE = LOWBLKSIZE;
	fprintf(stderr,"*warning* setting -k [%zd-%zd] because of the alignment of %zd bytes\n", LOWBLKSIZE/1024, BLKSIZE/1024, alignment);
      }


      free(sched); sched = NULL;
      free(suffix); suffix = NULL;
      
      if (readRatio < 1) { // if any writes
	if (dontUseExclusive < 3) { // specify at least -XXX to turn off exclusive
	  fdArray[i] = open(newpath, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
	} else {
	  fprintf(stderr,"  *warning* opening %s without O_EXCL\n", newpath);
	  fdArray[i] = open(newpath, O_RDWR | O_DIRECT | O_TRUNC);
	}
      } else { // only reads
	fdArray[i] = open(newpath, O_RDONLY | O_DIRECT); // if no writes, don't need write access
      }
      if (fdArray[i] < 0) {
	perror(newpath); goto cont;
      }

      //      fprintf(stderr,"nnn %s\n", newpath);
      actualBlockDeviceSize = blockDeviceSize(newpath);
      //      fprintf(stderr,"nnn %s\n", newpath);
      if (verbose >= 2) 
	fprintf(stderr,"*info* block device: '%s' size %zd bytes (%.1lf GiB)\n", newpath, actualBlockDeviceSize, TOGiB(actualBlockDeviceSize));

      if (sendTrim) {
	if (maxSizeGB > 0) {
	  trimDevice(fdArray[i], newpath, 0, maxSizeGB*1024*1024*1024);
	} else {
	  fprintf(stderr,"*info* limiting to 10GiB... ");
	  trimDevice(fdArray[i], newpath, 0, 10L*1024*1024*1024);
	}
      }

    } else {
      fdArray[i] = open(newpath, O_RDWR | O_DIRECT | O_EXCL); // if a file
      if (fdArray[i] < 0) {
	perror(newpath); goto cont;
      }

      actualBlockDeviceSize = fileSize(fdArray[i]);
      fprintf(stderr,"*info* file: '%s' size %zd bytes (%.1lf GiB)\n", newpath, actualBlockDeviceSize, TOGiB(actualBlockDeviceSize));
    }
    if (i == 0) {
      retBD = actualBlockDeviceSize;
    } else {
      if (actualBlockDeviceSize < retBD) {
	retBD = actualBlockDeviceSize;
      }
    }
    
    (*fdLen)++;

  cont: {}
    
  } // i

  /*  if (error) {
    exit(1);
    }*/

  return retBD;
}

  

int main(int argc, char *argv[]) {

  handle_args(argc, argv);
  if (exitAfterSeconds < 0) {
    exitAfterSeconds = 99999999;
  }

  diskStatType dst; // count sectors/bytes
  diskStatSetup(&dst);
  if (specifiedDevices) {
    diskStatFromFilelist(&dst, specifiedDevices, verbose);
  }
  
  int *fdArray;
  size_t fdLen = 0;
  CALLOC(fdArray, pathLen, sizeof(size_t));
  size_t actualBlockDeviceSize = openArrayPaths(pathArray, pathLen, fdArray, &fdLen, sendTrim, maxSizeGB);

  if (alignment == 0) {
    alignment = LOWBLKSIZE;
  }

  // using the -G option to reduce the max position on the block device
  size_t bdSize = actualBlockDeviceSize;
  const size_t origBDSize = bdSize;
  if (maxSizeGB > 0) {
    bdSize = (size_t) (maxSizeGB * 1024L * 1024 * 1024);
  }
  
  if (bdSize > actualBlockDeviceSize) {
    bdSize = actualBlockDeviceSize;
    fprintf(stderr,"*info* override option too high, reducing size to %.1lf GiB\n", TOGiB(bdSize));
  } else if (bdSize < actualBlockDeviceSize) {
    fprintf(stderr,"*info* size limited %.4lf GiB (original size %.2lf GiB)\n", TOGiB(bdSize), TOGiB(actualBlockDeviceSize));
  }

  srand48(seed);
  fprintf(stderr,"*info* seed = %ld\n", seed);


  char *randomBuffer = aligned_alloc(alignment, BLKSIZE); if (!randomBuffer) {fprintf(stderr,"oom!\n");exit(1);}
  generateRandomBuffer(randomBuffer, BLKSIZE);

  positionType *positions = createPositions(maxPositions);

  int exitcode = 0;
  
  size_t row = 0;
  if (table) {
    // generate a table
    size_t bsArray[]={BLKSIZE};
    double rrArray[]={1.0, 0, 0.5};

    size_t *qdArray = NULL;
    if (qdSpecified) {
      qdSpecified = 1;
      CALLOC(qdArray, qdSpecified, sizeof(size_t));
      qdArray[0] = qd;
    } else {
      qdSpecified = 4;
      CALLOC(qdArray, qdSpecified, sizeof(size_t));
      qdArray[0] = 1; qdArray[1] = 8; qdArray[2] = 32; qdArray[3] = 256;
    }
      
    size_t *ssArray = NULL; 
    if (seqFilesSpecified) { // if 's' specified on command line, then use it only 
      seqFilesSpecified = 1;
      CALLOC(ssArray, seqFilesSpecified, sizeof(size_t));
      ssArray[0] = seqFiles;
    } else { // otherwise an array of values
      seqFilesSpecified = 5;
      CALLOC(ssArray, seqFilesSpecified, sizeof(size_t));
      ssArray[0] = 0; ssArray[1] = 1; ssArray[2] = 8; ssArray[3] = 32; ssArray[4] = 128;
    }

    fprintf(stderr," blkSz\t numSq\tQueueD\t   R/W\t  IOPS\t MiB/s\t Ampli\t Disk%%\n");
    
    for (size_t rrindex=0; rrindex < sizeof(rrArray) / sizeof(rrArray[0]); rrindex++) {
      for (size_t ssindex=0; ssindex < seqFilesSpecified; ssindex++) {
	for (size_t qdindex=0; qdindex < qdSpecified; qdindex++) {
	  for (size_t bsindex=0; bsindex < sizeof(bsArray) / sizeof(bsArray[0]); bsindex++) {
	    size_t rb = 0, ios = 0, totalWB = 0, totalRB = 0;
	    double start = 0, elapsed = 0;
	    char filename[1024];

	    if (logFNPrefix) {
	      mkdir(logFNPrefix, 0755);
	    }
	    sprintf(filename, "%s/bs%zd_ss%zd_qd%zd_rr%.2f", logFNPrefix ? logFNPrefix : ".", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    logSpeedType l;
	    logSpeedInit(&l);

	    diskStatStart(&dst); // reset the counts
	    
	    fprintf(stderr,"%6zd\t%6zd\t%6zd\t%6.2f\t", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    
	    if (ssArray[ssindex] == 0) {
	      // setup random positions. An value of 0 means random. e.g. zero sequential files
	      setupPositions(positions, &maxPositions, fdArray, fdLen, bdSize, 0, rrArray[rrindex], bsArray[bsindex], bsArray[bsindex], alignment, singlePosition, jumpStep, startAtZero);

	      start = timedouble(); // start timing after positions created
	      rb = aioMultiplePositions(positions, maxPositions, exitAfterSeconds, qdArray[qdindex], 0, 1, &l, randomBuffer, bsArray[bsindex], alignment, &ios, &totalRB, &totalWB);
	    } else {
	      // setup multiple/parallel sequential region
	      setupPositions(positions, &maxPositions, fdArray, fdLen, bdSize, ssArray[ssindex], rrArray[rrindex], bsArray[bsindex], bsArray[bsindex], alignment, singlePosition, jumpStep, startAtZero);

	      
	      start = timedouble(); // start timing after positions created
	      rb = aioMultiplePositions(positions, maxPositions, exitAfterSeconds, qdArray[qdindex], 0, 1, &l, randomBuffer, bsArray[bsindex], alignment, &ios, &totalRB, &totalWB);
	    }
	    if (verbose) {
	      fprintf(stderr,"*info* calling fsync()\n");
	    }
	    for (size_t f = 0; f < fdLen; f++) {
	      fsync(fdArray[f]); // should be parallel sync
	    }

	    elapsed = timedouble() - start;
	      
	    diskStatFinish(&dst);

	    size_t trb = 0, twb = 0;
	    double util = 0;
	    diskStatSummary(&dst, &trb, &twb, &util, 0, 0, 0, elapsed);	    
	    size_t shouldHaveBytes = rb;
	    size_t didBytes = trb + twb;
	    double efficiency = didBytes *100.0/shouldHaveBytes;
	    if (!specifiedDevices) {
	      efficiency = 100;
	    }

	    logSpeedDump(&l, filename);
	    logSpeedFree(&l);
	    
	    fprintf(stderr,"%6.0lf\t%6.0lf\t%6.0lf\t%6.0lf\n", ios/elapsed, TOMiB(ios*BLKSIZE/elapsed), efficiency, util);
	    row++;
	    if (row > 1) {
	      //	      rrindex=99999;ssindex=99999;qdindex=99999;bsindex=99999;
	    }
	  }
	}
      }
    }
    free(ssArray); ssArray = NULL;

    // end table results
  } else {
    // just execute a single run
    size_t totl = diskStatTotalDeviceSize(&dst);
    fprintf(stderr,"*info* readWriteRatio: %.2lf, QD: %d, block size: %zd-%zd KiB (aligned to %zd bytes)\n", readRatio, qd, LOWBLKSIZE/1024, BLKSIZE/1024, alignment);
    fprintf(stderr,"*info* flushEvery %d, max bdSize %.3lf GiB\n", flushEvery, TOGiB(bdSize));
    if (totl > 0) {
      fprintf(stderr,"*info* origBDSize %.3lf GiB, sum rawDiskSize %.3lf GiB (overhead %.1lf%%)\n", TOGiB(origBDSize), TOGiB(totl), 100.0*totl/origBDSize - 100);
    }
    setupPositions(positions, &maxPositions, fdArray, fdLen, bdSize, seqFiles, readRatio, LOWBLKSIZE, BLKSIZE, alignment, singlePosition, jumpStep, startAtZero);

    if (logPositions) {
      if (verbose) {
	fprintf(stderr, "*info* dumping positions to '%s'\n", logPositions);
      }
      dumpPositions(logPositions, positions, maxPositions, bdSize);
    }

    diskStatStart(&dst); // grab the sector counts
    double start = timedouble();

    size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
    aioMultiplePositions(positions, maxPositions, exitAfterSeconds, qd, verbose, 0, NULL, randomBuffer, BLKSIZE, alignment, &ios, &shouldReadBytes, &shouldWriteBytes);
    if (verbose) {
      fprintf(stderr,"*info* calling fsync()\n");
    }
    for (size_t f = 0; f < fdLen; f++) {
      fsync(fdArray[f]); // should be parallel sync
    }
    
    double elapsed = timedouble() - start;
    
    diskStatFinish(&dst); // and sector counts when finished

    char s[1000];
    sprintf(s, "total read %.2lf GiB, %.0lf MiB/s, total write = %.2lf GiB, %.0lf MiB/s, %.1lf s, qd %d, bs %zd-%zd, seq %d, drives %zd, testSize %.2lf GiB",
	    TOGiB(shouldReadBytes), TOMiB(shouldReadBytes)/elapsed,
	    TOGiB(shouldWriteBytes), TOMiB(shouldWriteBytes)/elapsed, elapsed, qd, LOWBLKSIZE, BLKSIZE, seqFiles, fdLen, TOGiB(actualBlockDeviceSize));
    fprintf(stderr,"*info* %s\n", s);

    char *user = username();
    syslog(LOG_INFO, "%s - %s stutools %s", s, user, VERSION);
    free(user);


    /* number of bytes read/written not under our control */
    size_t trb = 0, twb = 0;
    double util = 0;
    diskStatSummary(&dst, &trb, &twb, &util, shouldReadBytes, shouldWriteBytes, 1, elapsed);

    // if we want to verify, we iterate through the successfully completed IO events, and verify the writes
    if (verifyWrites && readRatio < 1) {
      int numerrors = aioVerifyWrites(fdArray, fdLen, positions, maxPositions, BLKSIZE, alignment, verbose, randomBuffer);
      if (numerrors) {
	exitcode = MIN(numerrors, 999);
      }
    }

    for (size_t f = 0; f < fdLen; f++) {
      close(fdArray[f]); // should be parallel sync
    }

  } // end single run

  diskStatFree(&dst);
  free(positions);
  free(randomBuffer);
  if (fdArray) free(fdArray);
  if (pathLen) {
    for(size_t i = 0; i < pathLen;i++)
      free(pathArray[i]);
  }
  if (pathArray) free(pathArray);
  if (logFNPrefix) free(logFNPrefix);
  if (specifiedDevices) free(specifiedDevices);
  if (logPositions) free(logPositions);
  
  return exitcode;
}
