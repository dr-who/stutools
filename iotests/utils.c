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

#include "utils.h"

extern int keeprunning;


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


void doChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int writeAction) {
  void *buf = NULL;
  if (posix_memalign(&buf, 4096, maxBufSize)) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  memset(buf, 0, maxBufSize);

  int wbytes = 0;
  size_t lastg = 0;
  int chunkIndex = 0;
  double startTime = timedouble();
  while (keeprunning) {
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
    
    l->total += wbytes;
    logSpeedAdd(l, wbytes);
    if ((l->total - lastg) >= outputEvery) {
      lastg = l->total;
      fprintf(stderr,"%s '%s': %.1lf GB, mean %.1f MB/s, median %.1f MB/s, 99%% %.1f MB/s (n=%zd)/s, %.1fs\n", writeAction ? "write" : "read", label, l->total / 1024.0 / 1024 / 1024, logSpeedMean(l) / 1024.0 / 1024, logSpeedMedian(l) / 1024.0 / 1024, logSpeed99(l) /1024.0/1024, l->num, timedouble() - l->starttime);
    }
    if (chunkIndex >= numChunks) {
      chunkIndex = 0;
    }
    if (timedouble() - startTime > maxTime) {
      fprintf(stderr,"timer triggered after %zd seconds\n", maxTime);
      break;
    }
  }
  fprintf(stderr,"finished. Total %s speed '%s': %.1lf GB bytes in %.1f seconds, mean %.2f MB/s, n=%zd\n", writeAction ? "write" : "read", label, l->total / 1024.0 / 1024 / 1024, logSpeedTime(l), logSpeedMean(l) / 1024.0 / 1024, logSpeedN(l));
  free(buf);
}



void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 1);
}

void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 0);
}


int isBlockDevice(char *name) {
  struct stat sb;

  if (stat(name, &sb) == -1) {
    return 0;
  }
  return (S_ISBLK(sb.st_mode));
}

