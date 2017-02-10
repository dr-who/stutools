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


void doChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int writeAction, int sequential) {
  void *buf = NULL;
  if (posix_memalign(&buf, 4096, 1024*1024+maxBufSize)) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  memset(buf, 0, maxBufSize);

  srand(fd);
  size_t maxblocks = 0;
  if (!sequential) {
    if (isBlockDevice(label)) {
       maxblocks = blockDeviceSize(label)/4096;
       fprintf(stderr,"max blocks on %s is %zd\n", label, maxblocks);
    }
  } 
  int wbytes = 0;
  size_t lastg = 0;
  int chunkIndex = 0;
  double startTime = timedouble();
  while (keeprunning) {
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
    if ((l->total - lastg) >= outputEvery) {
      lastg = l->total;
      fprintf(stderr,"%s '%s': %.1lf GiB, mean %.1f MiB/s, median %.1f MiB/s, 99%% %.1f MiB/s, n=%zd, %.1fs\n", writeAction ? "write" : "read", label, l->total / 1024.0 / 1024 / 1024, logSpeedMean(l) / 1024.0 / 1024, logSpeedMedian(l) / 1024.0 / 1024, logSpeed99(l) /1024.0/1024, l->num, timedouble() - l->starttime);
    }
    if (chunkIndex >= numChunks) {
      chunkIndex = 0;
    }
    if (timedouble() - startTime > maxTime) {
      //fprintf(stderr,"timer triggered after %zd seconds\n", maxTime);
      break;
    }
  }
  fprintf(stderr,"finished. Total %s speed '%s': %.1lf GiB bytes in %.1f seconds, mean %.2f MiB/s, n=%zd\n", writeAction ? "write" : "read", label, l->total / 1024.0 / 1024 / 1024, logSpeedTime(l), logSpeedMean(l) / 1024.0 / 1024, logSpeedN(l));
  free(buf);
}



void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 1, seq);
}

void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, l, maxBufSize, outputEvery, 0, seq);
}


int isBlockDevice(char *name) {
  struct stat sb;

  if (stat(name, &sb) == -1) {
    return 0;
  }
  return (S_ISBLK(sb.st_mode));
}

