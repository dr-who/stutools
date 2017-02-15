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


void doChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int writeAction, int sequential, int direct) {
  void *buf = NULL;
  if (posix_memalign(&buf, 4096, 1024*1024+maxBufSize)) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  memset(buf, 0, maxBufSize);

  srand(fd);
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
  double lastg = 0;
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
      wbytes = write(fd, buf, chunkSizes[chunkIndex++]);
    } else {
      wbytes = read(fd, buf, chunkSizes[chunkIndex++]);
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
  }
  if (writeAction) {
    fprintf(stderr,"flushing and closing...\n");
  }
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
  free(buf);
  logSpeedFree(&previousSpeeds);
}



void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 1, seq, direct);
}

void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 0, seq, direct);
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
