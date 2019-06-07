#ifndef _DISKSTATS_H
#define _DISKSTATS_H

#include <unistd.h>

typedef struct {
  size_t startSecRead;
  size_t startSecWrite;
  size_t startSecTimeio;
  size_t startIORead;
  size_t startIOWrite;
  
  size_t finishSecRead;
  size_t finishSecWrite;
  size_t finishSecTimeio;
  size_t finishIORead;
  size_t finishIOWrite;


  size_t numDevices;
  size_t allocDevices;
  int *majorArray;
  int *minorArray;
  size_t *sizeArray;
} diskStatType;

void diskStatSetup(diskStatType *d);
void diskStatAddStart(diskStatType *d, size_t readSectors, size_t writeSectors);
void diskStatAddFinish(diskStatType *d, size_t readSectors, size_t writeSectors);
void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t *totalReadIO, size_t *totalWriteIO, double *util, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose, double elapsed);
void diskStatAddDrive(diskStatType *d, int fd);
void diskStatSectorUsage(diskStatType *d, size_t *sread, size_t *swritten, size_t *stimeio, size_t *ioread, size_t *iowrite, int verbose);
void diskStatFromFilelist(diskStatType *d, const char *path, int verbose);
void diskStatStart(diskStatType *d);
void diskStatFinish(diskStatType *d);
void diskStatFree(diskStatType *d);
size_t diskStatTotalDeviceSize(diskStatType *d);

void getProcDiskstats(const unsigned int major, const unsigned int minor, size_t *sread, size_t *swritten, size_t *stimeIO, size_t *readscompl, size_t *writecompl);
size_t diskStatTBRead(diskStatType *d);
size_t diskStatTBWrite(diskStatType *d);
void diskStatRestart(diskStatType *d);

#endif

