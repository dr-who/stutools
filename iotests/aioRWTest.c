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
#include "diskStats.h"

int    keepRunning = 1;       // have we been interrupted
double exitAfterSeconds = 5;
int    qd = 32;
int    qdSpecified = 0;
char   *path = NULL;
int    seqFiles = 0;
int    seqFilesSpecified = 0;
double maxSizeGB = 0;
size_t BLKSIZE=65536;
size_t jumpStep = 1;
double readRatio = 0.5;
size_t table = 0;
char   *logFNPrefix = NULL;
int    verbose = 0;
int    singlePosition = 0;
int    flushWhenQueueFull = 0;
size_t noops = 1;
int    verifyWrites = 0;
char*  specifiedDevices = NULL;

void handle_args(int argc, char *argv[]) {
  int opt;
  long int seed = (long int) timedouble();
  if (seed < 0) seed=-seed;
  
  while ((opt = getopt(argc, argv, "dDt:k:o:q:f:s:G:j:p:Tl:vVSF0R:O:rwb:")) != -1) {
    switch (opt) {
    case 'T':
      table = 1;
      break;
    case 'O':
      specifiedDevices = strdup(optarg);
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
      if (flushWhenQueueFull == 0) {
	flushWhenQueueFull = 1;
      } else {
	flushWhenQueueFull = 10 * flushWhenQueueFull;
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
      exitAfterSeconds = atof(optarg); if (exitAfterSeconds < 0.1) exitAfterSeconds = 0.1;
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
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0          # 50/50 read/write test, defaults to random\n");
    fprintf(stderr,"  ./aioRWTest -r -f /dev/nbd0       # read test, defaults to random\n");
    fprintf(stderr,"  ./aioRWTest -w -f /dev/nbd0       # write test, defaults to random\n");
    fprintf(stderr,"  ./aioRWTest -r -s1 -f /dev/nbd0   # read test, single contiguous region\n");
    fprintf(stderr,"  ./aioRWTest -w -s128 -f /dev/nbd0 # write test, 128 parallel contiguous region\n");
    fprintf(stderr,"  ./aioRWTest -S -F -f /dev/nbd0    # single static position, fsync after every op\n");
    fprintf(stderr,"  ./aioRWTest -p0.25 -f /dev/nbd0   # 25%% write, and 75%% read\n");
    fprintf(stderr,"  ./aioRWTest -p1 -f /dev/nbd0 -k4 -q64 -s32 -j16  # 100%% reads over entire block device\n");
    fprintf(stderr,"  ./aioRWTest -p1 -f /dev/nbd0 -k4 -q64 -s32 -j16  # 100%% reads over entire block device\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -G100    # limit actions to first 100GiB\n");
    fprintf(stderr,"  ./aioRWTest -p0.1 -f/dev/nbd0 -G3 # 90%% reads, 10%% writes, limited to first 3GiB of device\n");
    fprintf(stderr,"  ./aioRWTest -t30 -f/dev/nbd0      # test for 30 seconds\n");
    fprintf(stderr,"  ./aioRWTest -S -S -F -F -k4 -f /dev/nbd0  # single position, changing every 10 ops, fsync every 10 ops\n");
    fprintf(stderr,"  ./aioRWTest -0 -F -f /dev/nbd0  # send no operations, then flush. Basically, fast flush loop\n");
    fprintf(stderr,"  ./aioRWTest -S -F -V -f /dev/nbd0  # verbose that shows every operation\n");
    fprintf(stderr,"  ./aioRWTest -S -F -V -f file.txt  # can also use a single file. Note the file will be destroyed.\n");
    fprintf(stderr,"  ./aioRWTest -v -t15 -p0.5 -f /dev/nbd0  # random positions, 50%% R/W, verified after 15 seconds.\n");
    fprintf(stderr,"  ./aioRWTest -v -t15 -p0.5 -R 9812 -f /dev/nbd0  # set the starting seed to 9812\n");
    fprintf(stderr,"  ./aioRWTest -s 1 -j 10 -f /dev/sdc -V   # contiguous access, jumping 10 blocks at a time\n");
    fprintf(stderr,"  ./aioRWTest -s -8 -f /dev/sdc -V    # reverse contiguous 8 regions in parallel\n");
    fprintf(stderr,"  ./aioRWTest -f /dev/nbd0 -O ok.txt  # use a list of devices in ok.txt for disk stats/amplification\n");
    fprintf(stderr,"\nTable summary:\n");
    fprintf(stderr,"  ./aioRWTest -T -t 2 -f /dev/nbd0  # table of various parameters\n");
    exit(1);
  }

  srand48(seed);
  fprintf(stderr,"*info* seed = %ld\n", seed);

}


// sorting function, used by qsort
static int poscompare(const void *p1, const void *p2)
{
  const positionType *pos1 = (positionType*)p1;
  const positionType *pos2 = (positionType*)p2;
  if (pos1->pos < pos2->pos) return -1;
  else if (pos1->pos > pos2->pos) return 1;
  else return 0;
}


positionType *createPositions(size_t num) {
  positionType *p;
  CALLOC(p, num, sizeof(positionType));
  return p;
}

// lots of checks
void dumpPositionStats(positionType *positions, size_t num, size_t bdSize) {
  size_t rcount = 0, wcount = 0;
  size_t sizelow = -1, sizehigh = 0;
  positionType *p = positions;
  for (size_t j = 0; j < num; j++) {
    if (p->len < sizelow) {
      sizelow = p->len;
    }
    if (p->len > sizehigh) {
      sizehigh = p->len;
    }
    if (p->action == 'R') {
      rcount++;
    } else {
      wcount++;
    }
    p++;
  }

  // check all positions are aligned to low and high lengths
  p = positions;
  if (sizelow > 0) {
    for (size_t j = 0; j < num; j++) {
      if (p->pos + sizehigh > bdSize) {
	fprintf(stderr,"eek off the end of the array %zd!\n", p->pos);
      }
      if ((p->pos % sizelow) != 0) {
	fprintf(stderr,"eek no %zd aligned position at %zd!\n", sizelow, p->pos);
      }
      if ((p->pos % sizehigh) != 0) {
	fprintf(stderr,"eek no %zd aligned position at %zd!\n", sizehigh, p->pos);
      }
      p++;
    }
  }

  // duplicate, sort the array. Count unique positions
  positionType *copy;
  CALLOC(copy, num, sizeof(positionType));
  memcpy(copy, positions, num * sizeof(positionType));
  qsort(copy, num, sizeof(positionType), poscompare);
  // check order
  size_t unique = 0;
  for (size_t i = 1; i <num; i++) {
    if (copy[i].pos != copy[i-1].pos) {
      unique++;
      if (i==1) { // if the first number checked is different then really it's 2 unique values.
	unique++;
      }
    }
    if (copy[i].pos < copy[i-1].pos) {
      fprintf(stderr,"not sorted %zd %zd, unique %zd\n",i, copy[i].pos, unique);
    }
  }
  free(copy);

  if (verbose) {
    fprintf(stderr,"action summary: reads %zd, writes %zd, checked ratio %.1lf, len [%zd, %zd], unique positions %zd\n", rcount, wcount, rcount*1.0/(rcount+wcount), sizelow, sizehigh, unique);
  }
}

// create the position array
void setupPositions(positionType *positions, size_t num, const size_t bdSize, const int sf, const double readorwrite, size_t bs) {
  if (bdSize < BLKSIZE) {
    fprintf(stderr,"*warning* size of device is less than block size!\n");
    return;
  }
  
  if (singlePosition) {
    size_t con = (lrand48() % (bdSize / BLKSIZE)) * BLKSIZE;
    fprintf(stderr,"Using a single block position: %zd (singlePosition value %d)\n", con, singlePosition);
    for (size_t i = 0; i < num; i++) {
      if (singlePosition > 1) {
	if ((i % singlePosition) == 0) {
	  con = (lrand48() % (bdSize / BLKSIZE)) * BLKSIZE;
	}
      }
      positions[i].pos = con;
    }
  } else {
    // random positions
    if (sf == 0) {
      for (size_t i = 0; i < num; i++) {
	positions[i].pos = (lrand48() % (bdSize / BLKSIZE)) * BLKSIZE;
      }
    } else {
      // parallel contiguous regions
      size_t *ppp = NULL;
      size_t gap = 0;
      int abssf = ABS(sf);
      gap = bdSize / abssf;
      if (gap == 0) {
	fprintf(stderr,"*error* serious problem!\n");
      }
      gap = (gap >> 16) <<16;
      CALLOC(ppp, abssf, sizeof(size_t));
      for (size_t i = 0; i < abssf; i++) {
	ppp[i] = i * gap;
      }
      for (size_t i = 0; i < num; i++) {
	// sequential
	positions[i].pos = ppp[i % abssf];
	ppp[i % abssf] += (jumpStep * BLKSIZE);
	if (ppp[i % abssf] + bs > bdSize) { // if could go off the end then set back to 0
	  ppp[i % abssf] = 0;
	}
      }
      free(ppp);

      if (sf < 0) {
	// reverse positions array
	fprintf(stderr,"*info* reversing positions\n");
	for (size_t i = 0; i < num/2; i++) { 
	  size_t temp = positions[i].pos;
	  positions[i].pos = positions[num-1 - i].pos;
	  positions[num-1 -i].pos = temp;
	}
      }
    }
  }

  // setup R/W
  positionType *p = positions;
  for (size_t j = 0; j < num; j++) {
    if (drand48() <= readorwrite) {
      p->action = 'R';
    } else {
      p->action = 'W';
    }
    p->len = BLKSIZE;
    p->success = 0;
    p++;
  }
  
  if (verbose) {
    for(size_t i = 0; i < MIN(num, 20);i++) {
      fprintf(stderr,"%zd: %c %zd %zd %d\n", i, positions[i].action, positions[i].pos, positions[i].len, positions[i].success);
    }
  }
  
  dumpPositionStats(positions, num, bdSize);
}


// the block size random buffer. Nice ASCII
void generateRandomBuffer(char *buffer, size_t size) {
  const char verystartpoint = ' ' + (lrand48() % 15);
  const char jump = (lrand48() % 3) + 1;
  char startpoint = verystartpoint;
  for (size_t j = 0; j < size; j++) {
    buffer[j] = startpoint;
    startpoint += jump;
    if (startpoint > 'z') {
      startpoint = verystartpoint;
    }
  }
  buffer[size] = 0; // end of string to help printing
  strncpy(buffer, "STU-EE!",7);
  if (strlen(buffer) != size) {
    fprintf(stderr,"eekk random!\n");
  }
  if (verbose >= 2) {
    fprintf(stderr,"generated randomBuffer: %s\n", buffer);
  }
}


int main(int argc, char *argv[]) {

  handle_args(argc, argv);

  int fd = 0;
  size_t actualBlockDeviceSize = 0;

  diskStatType dst; // count sectors/bytes
  diskStatSetup(&dst);

  if (isBlockDevice(path)) {
    actualBlockDeviceSize = blockDeviceSize(path);
    fd = open(path, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
    if (fd < 0) {
      perror(path);exit(1);
    }
    trimDevice(fd, path);

    if (specifiedDevices) {
      diskStatFromFilelist(&dst, specifiedDevices);
    } else {
      diskStatAddDrive(&dst, fd);
    }
    
  } else {
    fd = open(path, O_RDWR | O_DIRECT | O_EXCL);
    if (fd < 0) {
      perror(path);exit(1);
    }
    actualBlockDeviceSize = fileSize(fd);
    fprintf(stderr,"*info* file specified: '%s' size %zd bytes\n", path, actualBlockDeviceSize);
  }


  // using the -G option to reduce the max position on the block device
  size_t bdSize = actualBlockDeviceSize;
  if (maxSizeGB > 0) {
    bdSize = (size_t) (maxSizeGB * 1024L * 1024 * 1024);
  }
  
  if (bdSize > actualBlockDeviceSize) {
    bdSize = actualBlockDeviceSize;
    fprintf(stderr,"*info* override option too high, reducing size to %.1lf GiB\n", TOGiB(bdSize));
  } else if (bdSize < actualBlockDeviceSize) {
    fprintf(stderr,"*info* size limited %.4lf GiB (original size %.2lf GiB)\n", TOGiB(bdSize), TOGiB(actualBlockDeviceSize));
  }

  
  const size_t num = noops * 10*1000*1000; // use 10M operations
  positionType *positions = createPositions(num);

  char *randomBuffer = aligned_alloc(4096, BLKSIZE + 1); if (!randomBuffer) {fprintf(stderr,"oom!\n");exit(1);}
  generateRandomBuffer(randomBuffer, BLKSIZE);

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
	    double ios = 0, start = 0, elapsed = 0;
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
	      setupPositions(positions, num, bdSize, 0, rrArray[rrindex], bsArray[bsindex]);

	      start = timedouble(); // start timing after positions created
	      ios = aioMultiplePositions(fd, positions, num, exitAfterSeconds, qdArray[qdindex], 0, 1, &l, randomBuffer);
	    } else {
	      // setup multiple/parallel sequential region
	      setupPositions(positions, num, bdSize, ssArray[ssindex], rrArray[rrindex], bsArray[bsindex]);
	      
	      start = timedouble(); // start timing after positions created
	      ios = aioMultiplePositions(fd, positions, num, exitAfterSeconds, qdArray[qdindex], 0, 1, &l, randomBuffer);
	    }
	    fsync(fd);
	    fdatasync(fd);
	    elapsed = timedouble() - start;
	      
	    diskStatFinish(&dst);

	    size_t trb = 0, twb = 0;
	    double util = 0;
	    diskStatSummary(&dst, &trb, &twb, &util, 0, 0, 0, elapsed);	    
	    size_t shouldHaveBytes = ios * bsArray[bsindex];
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
    free(ssArray);

    // end table results
  } else {
    // just execute a single run
    size_t totl = diskStatTotalDeviceSize(&dst);
    fprintf(stderr,"path: %s, readWriteRatio: %.2lf, QD: %d, blksz: %zd", path, readRatio, qd, BLKSIZE);
    fprintf(stderr,", bdSize %.3lf GiB, rawSize %.3lf GiB (overhead %.1lf%%)\n", TOGiB(bdSize), TOGiB(totl), 100.0*totl/bdSize - 100);
    setupPositions(positions, num, bdSize, seqFiles, readRatio, BLKSIZE);

    diskStatStart(&dst); // grab the sector counts
    double start = timedouble();

    aioMultiplePositions(fd, positions, num, exitAfterSeconds, qd, verbose, 0, NULL, randomBuffer);
    fsync(fd);
    close(fd);
    double elapsed = timedouble() - start;
    
    diskStatFinish(&dst); // and sector counts when finished
    
    /* total number of bytes read and written under our control */
    size_t shouldReadBytes = 0, shouldWriteBytes = 0;
    for (size_t i = 0; i < num; i++) {
      if (positions[i].success) {
	if (positions[i].action == 'W') shouldWriteBytes += positions[i].len;
	else shouldReadBytes += positions[i].len;
      }
    }
    if (verbose) {
      fprintf(stderr,"*info* total reads = %zd, total writes = %zd\n", shouldReadBytes, shouldWriteBytes);
    }

    /* number of bytes read/written not under our control */
    size_t trb = 0, twb = 0;
    double util = 0;
    diskStatSummary(&dst, &trb, &twb, &util, shouldReadBytes, shouldWriteBytes, 1, elapsed);

    // if we want to verify, we iterate through the successfully completed IO events, and verify the writes
    if (verifyWrites && readRatio < 1) {
      aioVerifyWrites(path, positions, num, BLKSIZE, verbose, randomBuffer);
    }
  } // end single run

  diskStatFree(&dst);
  free(positions);
  free(randomBuffer);
  if (logFNPrefix) {free(logFNPrefix);}
  if (specifiedDevices) {free(specifiedDevices);}
  
  return 0;
}
