#ifndef _DISKSTATS_H
#define _DISKSTATS_H

#include <unistd.h>

typedef struct {
  size_t startSecRead;
  size_t finisSecRead;
  size_t startSecWrite;
  size_t finisSecWrite;
} diskStatType;

void diskStatSetup(diskStatType *d);
void diskStatAddStart(diskStatType *d, size_t readSectors, size_t writeSectors);
void diskStatAddFinish(diskStatType *d, size_t readSectors, size_t writeSectors);
void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose);

#endif

