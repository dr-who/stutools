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

int    keepRunning = 1;       // have we been interrupted
double exitAfterSeconds = 2;
int    qd = 32;
char   *path = NULL;
int    seqFiles = 0;
double maxSizeGB = 0;
int    BLKSIZE=65536;
size_t jumpStep = 1;
double readRatio = 1.0;
size_t table = 0;
char   *logFNPrefix = NULL;
int    verbose = 0;
int    singlePosition = 0;
int    flushWhenQueueFull = 0;
size_t noops = 1;

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDr:t:k:o:q:f:s:G:j:p:Tl:vSF0")) != -1) {
    switch (opt) {
    case 'T':
      table = 1;
      break;
    case '0':
      noops = 0;
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
    fprintf(stderr,"  ./aioRWTest -p1 -f /dev/nbd0 -k4 -q64 -s32 -j16  # 100%% reads over entire block device\n");
    fprintf(stderr,"  ./aioRWTest -p0.75 -f /dev/nbd0 -k4 -q64 -s32 -j16 -G100 # 75%% reads, limited to the first 100GB\n");
    fprintf(stderr,"  ./aioRWTest -p0.0 -f /dev/nbd0 -k4 -q64 -s32 -j16 -G100  # 0%% reads, 100%% writes\n");
    fprintf(stderr,"  ./aioRWTest -S -F -k4 -f /dev/nbd0  # single position, fsync after every op\n");
    fprintf(stderr,"  ./aioRWTest -S -S -F -F -k4 -f /dev/nbd0  # single position, changing every 10 ops, fsync every 10 ops\n");
    fprintf(stderr,"  ./aioRWTest -0 -F -f /dev/nbd0  # send no operations, then flush. Basically, fast flush loop\n");
    fprintf(stderr,"  ./aioRWTest -S -F -v -f /dev/nbd0  # verbose that shows every operation\n");
    fprintf(stderr,"  ./aioRWTest -S -F -v -f file.txt  # can also use a single file. Note the file will be destroyed.\n");
    fprintf(stderr,"\nTable summary:\n");
    fprintf(stderr,"  ./aioRWTest -T -t 2 -f /dev/nbd0  # table of various parameters\n");
    exit(1);
  }
}


void setupPositions(size_t *positions, size_t num, const size_t bdSize, const int sf) {
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
	  
      positions[i] = con;
    }
  } else {
    // dynamic positions
    if (sf == 0) {
      for (size_t i = 0; i < num; i++) {
	positions[i] = (lrand48() % (bdSize / BLKSIZE)) * BLKSIZE;
      }
    } else {
      size_t *ppp = NULL;
      size_t gap = 0;
      gap = bdSize / (sf);
      gap = (gap >> 16) <<16;
      ppp = calloc(sf, sizeof(size_t));
      for (size_t i = 0; i < sf; i++) {
	ppp[i] = i * gap;
      }
      for (size_t i = 0; i < num; i++) {
	// sequential
	positions[i] = ppp[i % sf];
	ppp[i % sf] += (jumpStep * BLKSIZE);
      }
      free(ppp);
    }
  }

  /*  if (verbose) {
    fprintf(stderr,"\n");
    for(size_t i = 0; i < 10;i++) {
      fprintf(stderr,"%zd: %zd\n", i, positions[i]);
    }
  }
  */

}


    
    


int main(int argc, char *argv[]) {
  handle_args(argc, argv);

  size_t seed = (size_t) timedouble();
  srand48(seed);
  int fd = 0;

  size_t origbdSize = 0;
  if (isBlockDevice(path)) {
    origbdSize = blockDeviceSize(path);
    fd = open(path, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
  } else {
    fd = open(path, O_RDWR | O_DIRECT | O_EXCL);
    origbdSize = fileSize(fd);
    fprintf(stderr,"*info* file specified: '%s' size %zd bytes\n", path, origbdSize);
  }
  if (fd < 0) {perror(path);return -1; }

  size_t bdSize = origbdSize;
  if (maxSizeGB > 0) {
    bdSize = (size_t) (maxSizeGB * 1024L * 1024 * 1024);
  }
  
  if (bdSize > origbdSize) {
    bdSize = origbdSize;
    fprintf(stderr,"*info* override option too high, reducing size to %.1lf GB\n", bdSize /1024.0/1024/1024);
  } else if (bdSize < origbdSize) {
    fprintf(stderr,"*info* size limited %.2lf GB (original size %.2lf GB)\n", bdSize / 1024.0/1024/1024, origbdSize /1024.0/1024/1024);
  }

  
  const size_t num = noops * 10*1000*1000;
  size_t *positions = malloc((num+1) * sizeof(size_t));


  if (table) {
    // generate a table
    size_t bsArray[]={BLKSIZE};
    size_t qdArray[]={1, 8, 32, 256};
    double rrArray[]={1.0, 0, 0.5};
    size_t ssArray[]={0, 1, 8, 32, 128};

    fprintf(stderr,"blockSz\tnumSeq\tQueueD\tR/W\tIOPS\tMiB/s\n");
    for (size_t rrindex=0; rrindex < sizeof(rrArray) / sizeof(rrArray[0]); rrindex++) {
      for (size_t ssindex=0; ssindex < sizeof(ssArray) / sizeof(ssArray[0]); ssindex++) {
	for (size_t qdindex=0; qdindex < sizeof(qdArray) / sizeof(qdArray[0]); qdindex++) {
	  for (size_t bsindex=0; bsindex < sizeof(bsArray) / sizeof(bsArray[0]); bsindex++) {
	    double ios = 0, start = 0, elapsed = 0;
	    char filename[1024];

	    if (logFNPrefix) {
	      mkdir(logFNPrefix, 0755);
	    }
	    sprintf(filename, "%s/bs%zd_ss%zd_qd%zd_rr%.2f", logFNPrefix ? logFNPrefix : ".", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    logSpeedType l;
	    logSpeedInit(&l);
	    
	    fprintf(stderr,"%zd\t%zd\t%zd\t%4.2f\t", bsArray[bsindex], ssArray[ssindex], qdArray[qdindex], rrArray[rrindex]);
	    
	    if (ssArray[ssindex] == 0) {
	      // random
	      start = timedouble();
	      setupPositions(positions, num, bdSize, 0);
	      ios = aioMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qdArray[qdindex], rrArray[rrindex], 0, 1, &l);
	      fsync(fd);
	      fdatasync(fd);
	      elapsed = timedouble() - start;
	    } else {
	      // setup multiple sequential positions
	      setupPositions(positions, num, bdSize, ssArray[ssindex]);
	      start = timedouble();
	      ios = aioMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qdArray[qdindex], rrArray[rrindex], 0, 1, &l);
	      fsync(fd);
	      fdatasync(fd);

	      elapsed = timedouble() - start;
	    }

	    logSpeedDump(&l, filename);
	    logSpeedFree(&l);
	    
	    fprintf(stderr,"%6.0lf\t%6.0lf\n", ios/elapsed, ios*BLKSIZE/elapsed/1024.0/1024.0);
	    fsync(fd);
	    fdatasync(fd);
	  }
	}
      }
    }
  } else {
    // just execute a single run
    fprintf(stderr,"path: %s, readRatio: %.2lf, max queue depth: %d, seed %zd, blocksize: %d", path, readRatio, qd, seed, BLKSIZE);
    fprintf(stderr,", bdSize %.1lf GB\n", bdSize/1024.0/1024/1024);
    if (seqFiles == 0) {
      setupPositions(positions, num, bdSize, 0);
      aioMultiplePositions(fd, positions, num, BLKSIZE, exitAfterSeconds, qd, readRatio, verbose, 0, NULL);
    } else {
      setupPositions(positions, num, bdSize, seqFiles);
      aioMultiplePositions(fd, positions,     num, BLKSIZE, exitAfterSeconds, qd, readRatio, verbose, 0, NULL);
    }
  }
  
  free(positions);
  if (logFNPrefix) {
    free(logFNPrefix);
  }
  
  return 0;
}
