#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#include "diskStats.h"
#include "utils.h"


void diskStatClear(diskStatType *d) {
  d->startSecRead = 0;
  d->startSecWrite = 0;
  d->finishSecRead = 0;
  d->finishSecWrite = 0;
}
  
void diskStatSetup(diskStatType *d) {
  diskStatClear(d);
  d->numDrives = 0;
  d->allocDrives = 10;
  d->majorArray = calloc(d->allocDrives, sizeof(int));
  d->minorArray = calloc(d->allocDrives, sizeof(int));
}

void diskStatAddDrive(diskStatType *d, int fd) {
  unsigned int major, minor;
  majorAndMinor(fd, &major, &minor);
  if (d->numDrives >= d->allocDrives) {
    d->allocDrives += 10;
    d->majorArray = realloc(d->majorArray, d->allocDrives * sizeof(int));
    d->minorArray = realloc(d->minorArray, d->allocDrives * sizeof(int));
  }
  d->majorArray[d->numDrives] = major;
  d->minorArray[d->numDrives] = minor;
  //  fprintf(stderr,"diskStatAddDrive fd %d, major %u, minor %u\n", fd, major, minor);
  d->numDrives++;
}

void diskStatAddStart(diskStatType *d, size_t readSectors, size_t writeSectors) {
  d->startSecRead += readSectors;
  d->startSecWrite += writeSectors;
}

void diskStatAddFinish(diskStatType *d, size_t readSectors, size_t writeSectors) {
  d->finishSecRead += readSectors;
  d->finishSecWrite += writeSectors;
}


void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose) {
  if (d->startSecRead > d->finishSecRead) {
    fprintf(stderr,"start more than fin!\n");
  } 
  if (d->startSecWrite > d->finishSecWrite) {
    fprintf(stderr,"start more than fin2!\n");
  } 
  *totalReadBytes = (d->finishSecRead - d->startSecRead) * 512L;
  *totalWriteBytes =(d->finishSecWrite - d->startSecWrite) * 512L;
  if (verbose && (shouldReadBytes || shouldWriteBytes)) {
    fprintf(stderr,"*info* read  amplification: should be %zd (%.3lf GiB), read  %zd (%.3lf GiB), %.2lf%%, %zd device(s)\n", shouldReadBytes, TOGiB(shouldReadBytes), *totalReadBytes, TOGiB(*totalReadBytes), *totalReadBytes*100.0/shouldReadBytes, d->numDrives);
    fprintf(stderr,"*info* write amplification: should be %zd (%.3lf GiB), write %zd (%.3lf GiB), %.2lf%%, %zd device(s)\n", shouldWriteBytes, TOGiB(shouldWriteBytes), *totalWriteBytes, TOGiB(*totalWriteBytes), *totalWriteBytes*100.0/shouldWriteBytes, d->numDrives);
  }
}



void diskStatSectorUsage(diskStatType *d, size_t *sread, size_t *swritten, int verbose) {
  *sread = 0;
  *swritten = 0;
  for (size_t i = 0; i < d->numDrives; i++) {
    size_t sr = 0, sw = 0;
    getProcDiskstats(d->majorArray[i], d->minorArray[i], &sr, &sw);
    if (verbose) {
      fprintf(stderr,"*info* major %d minor %d sectorsRead %zd sectorsWritten %zd\n", d->majorArray[i], d->minorArray[i], sr, sw);
    }
    *sread = (*sread) + sr;
    *swritten = (*swritten) + sw;
  }
}

void diskStatFromFilelist(diskStatType *d, const char *path) {
  FILE *fp = fopen(path, "rt");
  if (!fp) {fprintf(stderr,"can't open %s!\n", path);exit(1);}

  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  char *str = malloc(1000); if (!str) {fprintf(stderr,"pd OOM\n");exit(1);}
  while ((read = getline(&line, &len, fp)) != -1) {
    if (sscanf(line, "%s", str) >= 1) {
      int fd = open(str, O_RDONLY);
      if (fd < 0) {
	perror("problem");
      }
      diskStatAddDrive(d, fd);
      close (fd);
    }
  }
  free(str);
  free(line);
  fclose(fp);
}


void diskStatStart(diskStatType *d) {
  diskStatClear(d);
  size_t sread = 0, swritten = 0;
  diskStatSectorUsage(d, &sread, &swritten, 0);
  d->startSecRead = sread;
  d->startSecWrite = swritten;
}

void diskStatFinish(diskStatType *d) {
  size_t sread = 0, swritten = 0;
  diskStatSectorUsage(d, &sread, &swritten, 0);
  d->finishSecRead = sread;
  d->finishSecWrite = swritten;
}
