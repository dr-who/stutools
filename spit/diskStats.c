#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <sys/sysmacros.h>

#include "diskStats.h"
#include "utils.h"


void diskStatClear(diskStatType *d) {
  d->startSecRead = 0;
  d->startSecWrite = 0;
  d->startSecTimeio = 0 ;
  d->finishSecRead = 0;
  d->finishSecWrite = 0;
  d->finishSecTimeio = 0;
}
  
void diskStatSetup(diskStatType *d) {
  diskStatClear(d);
  d->numDevices = 0;
  d->allocDevices = 10;
  CALLOC(d->majorArray, d->allocDevices, sizeof(int));
  CALLOC(d->minorArray, d->allocDevices, sizeof(int));
  CALLOC(d->sizeArray, d->allocDevices, sizeof(size_t));
}

void diskStatAddDrive(diskStatType *d, int fd) {
  unsigned int major = 0, minor = 0;
  majorAndMinor(fd, &major, &minor);
  if (d->numDevices >= d->allocDevices) {
    d->allocDevices += 10;
    d->majorArray = realloc(d->majorArray, d->allocDevices * sizeof(int));
    d->minorArray = realloc(d->minorArray, d->allocDevices * sizeof(int));
    d->sizeArray = realloc(d->sizeArray, d->allocDevices * sizeof(size_t));
  }
  d->majorArray[d->numDevices] = major;
  d->minorArray[d->numDevices] = minor;
  d->sizeArray[d->numDevices] = blockDeviceSizeFromFD(fd);
  //    fprintf(stderr,"diskStatAddDrive fd %d, major %u, minor %u\n", fd, major, minor);
  d->numDevices++;
}

void diskStatAddStart(diskStatType *d, size_t readSectors, size_t writeSectors) {
  d->startSecRead += readSectors;
  d->startSecWrite += writeSectors;
}

void diskStatAddFinish(diskStatType *d, size_t readSectors, size_t writeSectors) {
  d->finishSecRead += readSectors;
  d->finishSecWrite += writeSectors;
}


void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, double *util, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose, double elapsed) {
  if (d->startSecRead > d->finishSecRead) {
    fprintf(stderr,"start more than fin!\n");
  } 
  if (d->startSecWrite > d->finishSecWrite) {
    fprintf(stderr,"start more than fin2!\n");
  } 
  *totalReadBytes = (d->finishSecRead - d->startSecRead) * 512L;
  *totalWriteBytes =(d->finishSecWrite - d->startSecWrite) * 512L;
  *util = 100.0 * ((d->finishSecTimeio - d->startSecTimeio)/1000.0 / d->numDevices) / elapsed;
  
  if (verbose && (shouldReadBytes || shouldWriteBytes)) {
    if (*totalReadBytes)
      fprintf(stderr,"*info* read  amplification: should be %zd (%.3lf GiB), read  %zd (%.3lf GiB), %.2lf%%, %zd device(s)\n", shouldReadBytes, TOGiB(shouldReadBytes), *totalReadBytes, TOGiB(*totalReadBytes), *totalReadBytes*100.0/shouldReadBytes, d->numDevices);
    if (*totalWriteBytes)
      fprintf(stderr,"*info* write amplification: should be %zd (%.3lf GiB), write %zd (%.3lf GiB), %.2lf%%, %zd device(s)\n", shouldWriteBytes, TOGiB(shouldWriteBytes), *totalWriteBytes, TOGiB(*totalWriteBytes), *totalWriteBytes*100.0/shouldWriteBytes, d->numDevices);
    if (d->numDevices)
      fprintf(stderr,"*info* total disk utilisation: %.1lf %% (devices = %zd)\n", *util, d->numDevices);
  }
}

size_t diskStatTotalDeviceSize(diskStatType *d) {
  size_t totl = 0;
  for (size_t i = 0; i < d->numDevices; i++) {
    totl += d->sizeArray[i];
  }
  return totl;
}



void diskStatSectorUsage(diskStatType *d, size_t *sread, size_t *swritten, size_t *stimeio, int verbose) {
  *sread = 0;
  *swritten = 0;
  *stimeio = 0;
  for (size_t i = 0; i < d->numDevices; i++) {
    size_t sr = 0, sw = 0, sio = 0;
    getProcDiskstats(d->majorArray[i], d->minorArray[i], &sr, &sw, &sio);
    if (verbose) {
      fprintf(stderr,"*info* major %d minor %d sectorsRead %zd sectorsWritten %zd\n", d->majorArray[i], d->minorArray[i], sr, sw);
    }
    *sread = (*sread) + sr;
    *swritten = (*swritten) + sw;
    *stimeio = (*stimeio) + sio;
  }
}

void diskStatFromFilelist(diskStatType *d, const char *path, int verbose) {
  FILE *fp = fopen(path, "rt");
  if (!fp) {fprintf(stderr,"can't open %s!\n", path);exit(1);}

  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  char *str;
  CALLOC(str, 1000, 1);
  while ((read = getline(&line, &len, fp)) != -1) {
    if (sscanf(line, "%s", str) >= 1) {
      if (isBlockDevice(str)) {
	int fd = open(str, O_RDONLY);
	if (fd < 0) {
	  perror("problem");
	}
	if (verbose) {
	  char *sched = getScheduler(str);
	  fprintf(stderr,"*info* scheduler for %s is [%s]\n", str, sched);
	  free(sched);
	}
	diskStatAddDrive(d, fd);
	close (fd);
      } else {
	fprintf(stderr,"*warning* %s is not a block device\n", str);
      }
    }
  }
  free(str);
  free(line);
  fclose(fp);
  fprintf(stderr,"*info* specifiedDevices from %s, numDevices is %zd\n", path, d->numDevices);
}


void diskStatStart(diskStatType *d) {
  diskStatClear(d);
  size_t sread = 0, swritten = 0, stimeio = 0;
  diskStatSectorUsage(d, &sread, &swritten, &stimeio, 0);
  d->startSecRead = sread;
  d->startSecWrite = swritten;
  d->startSecTimeio = stimeio;
}

void diskStatFinish(diskStatType *d) {
  size_t sread = 0, swritten = 0, stimeio = 0;
  diskStatSectorUsage(d, &sread, &swritten, &stimeio, 0);
  d->finishSecRead = sread;
  d->finishSecWrite = swritten;
  d->finishSecTimeio = stimeio;
}

void diskStatFree(diskStatType *d) {
  if (d->majorArray) {free(d->majorArray); d->majorArray = NULL;}
  if (d->minorArray) {free(d->minorArray); d->minorArray = NULL;}
  if (d->sizeArray) {free(d->sizeArray); d->sizeArray = NULL;}
  diskStatClear(d);
  d->numDevices = 0;
  d->allocDevices = 0;
}


void majorAndMinor(int fd, unsigned int *major, unsigned int *minor) {
  struct stat buf;
  if (fstat(fd, &buf) == 0) {
    dev_t dt = buf.st_rdev;
    *major = major(dt);
    *minor = minor(dt);
  } else {
    fprintf(stderr,"*warning* can't get major/minor\n");
  }
}
  
void getProcDiskstats(const unsigned int major, const unsigned int minor, size_t *sread, size_t *swritten, size_t *stimeIO) {
  FILE *fp = fopen("/proc/diskstats", "rt");
  if (!fp) {
    fprintf(stderr,"can't open diskstats!\n");
    return;
  }
  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  char *str;
  CALLOC(str, 1000, 1);
  while ((read = getline(&line, &len, fp)) != -1) {
    long mj, mn, s;
    size_t read1, write1, timespentIO;
    sscanf(line,"%ld %ld %s %ld %ld %zu %ld %ld %ld %zu %ld %ld %zu", &mj, &mn, str, &s, &s, &read1, &s, &s, &s, &write1, &s, &s, &timespentIO);
    if (mj == major && mn == minor) {
      *sread = read1;
      *swritten = write1;
      *stimeIO = timespentIO;
      break;
      //      printf("Retrieved line of length (%u %u) (%zd %zd) :\n", mj, mn, *sread, *swritten);
      //      printf("%s", line);
    }
  }
  free(str);
  free(line);
  fclose(fp);
}

