#ifndef _UTILS_H
#define _UTILS_H

#include <malloc.h>
#include <string.h>

#include "logSpeed.h"

#define MIN(x,y) (((x) < (y)) ? (x) : (y))
#define MAX(x,y) (((x) > (y)) ? (x) : (y))
#define ABS(x) (((x) > 0) ? (x) : (-(x)))
#define TOGiB(x) ((x)/1024.0/1024/1024)
#define TOMiB(x) ((x)/1024.0/1024)

/*#define CALLOC(x, y, z) {x = calloc(y, z); if (!(x)) {fprintf(stderr,"ooom!!\n");abort();}}*/
#define CALLOC(x, y, z) {x = memalign(4096, (((size_t)((y) * (z)))/4096 + 1) * 4096); if(x) memset(x, 0, (((size_t)((y) * (z)))/4096 + 1) * 4096);  if (!(x)) {fprintf(stderr,"ooom!!\n");abort();}}


#define DIFF(x,y) ((x) > (y)) ? ((x)-(y)) : ((y) - (x))

double timeAsDouble();

void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess, int verifyWrites, float flushEverySecs);
void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess);

size_t blockDeviceSize(const char *name);
int isBlockDevice(const char *name);
size_t blockDeviceSizeFromFD(const int fd);


void dropCaches();
char *username();

char* queueType(const char *path);

void checkContents(char *label, char *charbuf, size_t size, const size_t checksum, float checkpercentage, size_t stopatbytes);
void shmemUnlink();

double loadAverage();
size_t numThreads();
size_t totalRAM();
char *OSRelease();
size_t swapTotal();

size_t fileSize(int fd);
size_t fileSizeFromName(const char *path);
size_t fileExists(const char *path);

void majorAndMinor(int fd, unsigned int *major, unsigned int *minor);

int getWriteCacheStatus(int fd);
int trimDevice(int fd, char *path, unsigned long low, unsigned long high);
void generateRandomBuffer(char *buffer, size_t size, long seed);
void generateRandomBufferCyclic(char *buffer, size_t size, long seed, size_t cyclic);


char *getSuffix(const char *path);
char *getScheduler(const char *path);

size_t alignedNumber(size_t num, size_t alignment);
size_t randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits);

  

void getPhyLogSizes(const char *suffix, size_t *phy, size_t *log);

int startsWith(const char *pre, const char *str);

#endif

