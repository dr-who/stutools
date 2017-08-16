#ifndef _UTILS_H
#define _UTILS_H

#include "logSpeed.h"

#define MIN(x,y) (((x) < (y)) ? (x) : (y))
#define ABS(x) (((x) > 0) ? (x) : (-(x)))
#define TOGiB(x) ((x)/1024.0/1024/1024)
#define TOMiB(x) ((x)/1024.0/1024)

#define CALLOC(x, y, z) {x = calloc(y, z); if (!(x)) {fprintf(stderr,"ooom!!\n");exit(1);}}

double timedouble();

void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess, int verifyWrites, float flushEverySecs);
void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess);

size_t blockDeviceSize(char *name);
int isBlockDevice(char *name);
size_t blockDeviceSizeFromFD(int fd);


void dropCaches();
char *username();

char* queueType(char *path);

void checkContents(char *label, char *charbuf, size_t size, const size_t checksum, float checkpercentage, size_t stopatbytes);
void shmemUnlink();

double loadAverage();
size_t numThreads();
size_t totalRAM();
char *OSRelease();

size_t fileSize(int fd);

void majorAndMinor(int fd, unsigned int *major, unsigned int *minor);

int getWriteCacheStatus(int fd);
int trimDevice(int fd, char *path, unsigned long low, unsigned long high);

#endif

