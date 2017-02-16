#define _GNU_SOURCE
#include <stdlib.h>
#include <fcntl.h>
#include <sys/time.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include <malloc.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <syslog.h>

#include "utils.h"

extern int keepRunning;


struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
}

double timedouble() {
  struct timeval now = gettm();
  double tm = (now.tv_sec * 1000000 + now.tv_usec);
  return tm/1000000.0;
}


size_t blockDeviceSize(char *path) {

  int fd = open(path, O_RDONLY );
  if (fd < 0) {
    perror(path);
    return 0;
  }
  size_t file_size_in_bytes = 0;
  ioctl(fd, BLKGETSIZE64, &file_size_in_bytes);
  close(fd);
  return file_size_in_bytes;
}


void doChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int writeAction, int sequential, int direct, int verifyWrites) {
  void *buf = aligned_alloc(65536, maxBufSize);
  if (!buf) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  srand((int)timedouble());

  char *charbuf = (char*) buf;
  size_t checksum = 0;
  const size_t startChar = rand() % 255;
  const size_t startMod = 20 + rand() % 40;
  for (size_t i = 0; i < maxBufSize; i++ ) {
    charbuf[i] = 'A' + (char) ((startChar + i ) % startMod);
    checksum += ('A' + (startChar + i) % startMod);
  }

  size_t maxblocks = 0;
  if (!sequential) {
    if (isBlockDevice(label) && ((maxblocks = blockDeviceSize(label)/4096) > 0)) {
       fprintf(stderr,"max blocks on %s is %zd\n", label, maxblocks);
    } else {
      fprintf(stderr,"error: need to be a block device with the -r option\n");
      exit(1);
    }
  } 
  int wbytes = 0;
  double lastg = timedouble();
  int chunkIndex = 0;
  double startTime = timedouble();
  int resetCount = 5;
  logSpeedType previousSpeeds;

  logSpeedInit(&previousSpeeds);
  
  while (keepRunning) {
    if (!sequential) {
	size_t pos = (rand() % maxblocks) * 4096;
 	//fprintf(stderr,"%zd\n", pos);	
 	off_t ret = lseek(fd, pos, SEEK_SET);
	if (ret < 0) {
	  perror("seek error");
	}
    }

    if (writeAction) {
      wbytes = write(fd, charbuf, chunkSizes[0]);
    } else {
      wbytes = read(fd, charbuf, chunkSizes[0]);
    }
    if (wbytes == 0) {
      break;
    }
    if (wbytes < 0) {
      perror("problem");
      break;
    }
    
    logSpeedAdd(l, wbytes);
    if ((timedouble() - lastg) >= outputEvery) {
      lastg = timedouble();
      fprintf(stderr,"%s '%s': %.1lf GiB, mean %.1f MiB/s, median %.1f MiB/s, 1%% %.1f MiB/s, 99%% %.1f MiB/s, n=%zd, %.1fs\n", writeAction ? "write" : "read", label, l->total / 1024.0 / 1024 / 1024, logSpeedMean(l) / 1024.0 / 1024, logSpeedMedian(l) / 1024.0 / 1024, logSpeedRank(l, 0.01) / 1024.0 / 1024, logSpeedRank(l, 0.99) /1024.0/1024, l->num, timedouble() - l->starttime);
      logSpeedAdd(&previousSpeeds, logSpeedMean(l));
      if (logSpeedN(&previousSpeeds) >= 10) { // at least 10 data points before a reset
	if (keepRunning) {
	  double low = logSpeedRank(&previousSpeeds, .1);
	  double high = logSpeedRank(&previousSpeeds, .9);
	  double mean = logSpeedMean(&previousSpeeds);
	  if ((high / low > 1.05) && (mean < low || mean > high) && (resetCount > 0)) { // must be over 5% difference
	    fprintf(stderr,"  [ %.1lf < %.1lf MiB/s < %.1lf ]\n", low / 1024.0 / 1024, mean / 1024.0 / 1024, high / 1024.0 /1024);
	    fprintf(stderr,"resetting due to volatile timings (%d resets remain)\n", resetCount);
	    resetCount--;
	    logSpeedReset(l);
	    logSpeedReset(&previousSpeeds);
	    startTime = timedouble();
	  }
	}
      }
    }
    if (chunkIndex >= numChunks) {
      chunkIndex = 0;
    }
    if (timedouble() - startTime > maxTime) {
      //fprintf(stderr,"timer triggered after %zd seconds\n", maxTime);
      break;
    }
  } // while loop
  if (writeAction) {
    fprintf(stderr,"flushing and closing...\n");
  }
  fdatasync(fd);
  fsync(fd);
  close(fd);
  l->lasttime = timedouble();
  if (resetCount > 0) {
    char s[1000];
    sprintf(s, "Total %s speed '%s': %.1lf GiB in %.1f s, mean %.2f MiB/s, %d bytes, %s, %s, n=%zd (stutools %s)%s\n", writeAction ? "write" : "read", label, l->total / 1024.0 / 1024 / 1024, logSpeedTime(l), logSpeedMean(l) / 1024.0 / 1024, chunkSizes[0], sequential ? "sequential" : "random", direct ? "O_DIRECT" : "pagecache", logSpeedN(l), VERSION, keepRunning ? "" : " - interrupted");
    fprintf(stderr, "%s", s);
    char *user = username();
    syslog(LOG_INFO, "%s - %s", user, s);
    free(user);
  } else {
    fprintf(stderr,"error: results too volatile. Perhaps the machine is busy?\n");
  }

  if (verifyWrites) {
    checkContents(label, charbuf, chunkSizes[0], checksum, 1);
  }
   
  free(buf);
  
  //  logSpeedHistogram(&previousSpeeds);
  logSpeedFree(&previousSpeeds);
}


void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, int verifyWrites) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 1, seq, direct, verifyWrites);
}

void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 0, seq, direct, 0);
}


int isBlockDevice(char *name) {
  struct stat sb;

  if (stat(name, &sb) == -1) {
    return 0;
  }
  return (S_ISBLK(sb.st_mode));
}


void dropCaches() {
  FILE *fp = fopen("/proc/sys/vm/drop_caches", "wt");
  if (fp == NULL) {
    fprintf(stderr,"error: you need sudo/root permission to drop caches\n");
    exit(1);
  }
  if (fprintf(fp, "3\n") < 1) {
    fprintf(stderr,"error: you need sudo/root permission to drop caches\n");
    exit(1);
  }
  fflush(fp);
  fclose(fp);
  fprintf(stderr,"caches dropped\n");
}


char *username() {
  char *buf = calloc(200, 1); if (buf == NULL) exit(-1);
  getlogin_r(buf, 200);
  return buf;
}


void checkContents(char *label, char *charbuf, size_t size, const size_t checksum, float checkpercentage) {
  fprintf(stderr,"verifying contents of '%s'...\n", label);
  int fd = open(label, O_RDONLY ); // O_DIRECT to check contents
  if (fd < 0) {
    perror(label);
    exit(1);
  }

  void *rawbuf = NULL;
  if ((rawbuf = aligned_alloc(4096, size)) == NULL) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  size_t pos = 0;
  unsigned char *buf = (unsigned char*)rawbuf;
  unsigned long ii = (unsigned long)rawbuf;
  size_t check = 0, ok = 0, error = 0;
  srand(ii);

  keepRunning = 1;
  while (keepRunning) {
    int wbytes = read(fd, buf, size);
    if (wbytes == 0) {
      break;
    }
    if (wbytes < 0) {
      perror("problem");
      break;
    }
    if (wbytes == size) { // only check the right size blocks
      check++;
      size_t newsum = 0;
      for (size_t i = 0; i <wbytes;i++) {
	newsum += (buf[i] & 0xff);
      }
      if (newsum != checksum) {
	error++;
	//	fprintf(stderr,"X");
	if (error < 5) {
	  fprintf(stderr,"checksum error %zd\n", pos);
	  //	  fprintf(stderr,"buffer: %s\n", buf);
	}
	if (error == 5) {
	  fprintf(stderr,"further errors not displayed\n");
	}
      } else {
	//	fprintf(stderr,".");
	ok++;
      }
    }
    pos += wbytes;
  }
  fflush(stderr);
  close(fd);

  char *user = username();
  syslog(LOG_INFO, "%s - verify '%s': blocks (%zd bytes) checked %zd, correct %zd, failed %zd\n", user, label, size, check, ok, error);

  fprintf(stderr, "verify '%s': blocks (%zd bytes) checked %zd, correct %zd, failed %zd\n", label, size, check, ok, error);

  if (error > 0) {
    syslog(LOG_INFO, "%s - checksum errors on '%s'\n", user, label);
    fprintf(stderr, "**CHECKSUM** errors\n");
  }
  free(user);
  free(rawbuf);
}

  
