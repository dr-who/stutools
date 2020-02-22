#ifndef _DISKSTATS_H
#define _DISKSTATS_H

#include <unistd.h>
#include <stdio.h>

typedef struct {
  size_t major;
  size_t minor;
  size_t secRead;
  size_t secWrite;
  size_t secTimeIO;
  size_t IORead;
  size_t IOWrite;
} devSnapshotType;

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

  size_t deviceCount;
  size_t deviceCountAlloc;
  devSnapshotType *deviceStats;
  
  size_t allocDevices;
  int *majorArray;
  int *minorArray;
} diskStatType;


void diskStatSetup(diskStatType *d);

void diskStatFree(diskStatType *d);

//void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t *totalReadIO, size_t *totalWriteIO, double *util, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose, double elapsed);

//void diskStatAddDrive(diskStatType *d, int fd);

void diskStatAddStart(diskStatType *d, size_t readSectors, size_t writeSectors);
void diskStatAddFinish(diskStatType *d, size_t readSectors, size_t writeSectors);
void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t *totalReadIO, size_t *totalWriteIO, double *util, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose, double elapsed);
void diskStatAddDrive(diskStatType *d, int fd);

void diskStatFromFilelist(diskStatType *d, const char *path, int verbose);

void diskStatStart(diskStatType *d);
void diskStatFinish(diskStatType *d);


//size_t diskStatTotalDeviceSize(diskStatType *d);

void getProcDiskstats(FILE *fp, const unsigned int major, const unsigned int minor, size_t *sread, size_t *swritten, size_t *stimeIO, size_t *readcompl, size_t *writecompl);

size_t diskStatTBRead(diskStatType *d);
size_t diskStatTBWrite(diskStatType *d);
void diskStatRestart(diskStatType *d);
void diskStatLoadProc(diskStatType *d);

void diskStatInfo(diskStatType *d);

void diskStatUsage(diskStatType *d, size_t *sread, size_t *swritten, size_t *stimeio, size_t *ioread, size_t *iowrite1);
size_t diskStatTBTimeSpentIO(diskStatType *d);

#endif

