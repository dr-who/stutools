#ifndef _UTILS_H
#define _UTILS_H

#include <malloc.h>
#include <string.h>

#include "logSpeed.h"

#define MIN(x,y) (((x) < (y)) ? (x) : (y))
#define MAX(x,y) (((x) > (y)) ? (x) : (y))
#define ABS(x) (((x) > 0) ? (x) : (-(x)))
#define TOPiB(x) ((x)/1024.0/1024/1024/1024/1024)
#define TOPB(x) ((x)/1000.0/1000/1000/1000/1000)

#define TOTB(x) ((x)/1000.0/1000/1000/1000)

#define TOGiB(x) ((x)/1024.0/1024/1024)
#define TOGB(x) ((x)/1000.0/1000/1000)

#define TOMiB(x) ((x)/1024.0/1024.0)
#define TOMB(x) ((x)/1000.0/1000.0)

#define TOKiB(x) ((x)/1024.0)

/*#define CALLOC(x, y, z) {x = calloc(y, z); if (!(x)) {fprintf(stderr,"ooom!!\n");abort();}}*/
#define CALLOC(x, y, z) {/*fprintf(stderr,"CALLOC %s %d, size %zd\n", __FILE__, __LINE__, ((((size_t)((y) * (z)))/4096 + 1) * 4096));*/ x = memalign(4096, (((size_t)((y) * (z)))/4096 + 1) * 4096); if(x) memset(x, 0, (((size_t)((y) * (z)))/4096 + 1) * 4096);  if (!(x)) {fprintf(stderr,"*error* out of memory! ooom!! %zd\n", (((size_t)((y) * (z)))/4096 + 1) * 4096);abort();}}


#define DIFF(x,y) ((x) > (y)) ? ((x)-(y)) : ((y) - (x))

double timedouble();

void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess, int verifyWrites, float flushEverySecs);
void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess);

size_t blockDeviceSize(const char *name);
int isBlockDevice(const char *name);
size_t blockDeviceSizeFromFD(const int fd);


void dropCaches();
char *username();
void usernameinit();

char* queueType(char *path);

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
void generateRandomBuffer(char *buffer, size_t size, unsigned short seed);
void generateRandomBufferCyclic(char *buffer, size_t size, unsigned short seed, size_t cyclic);


char *getSuffix(const char *path);
char *getScheduler(const char *path);

size_t alignedNumber(size_t num, size_t alignment);
size_t randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits, const size_t randomValue);

  

void getPhyLogSizes(const char *suffix, size_t *phy, size_t *log);

int startsWith(const char *pre, const char *str);
int canOpenExclusively(const char *fn);
int createFile(const char *filename, const size_t sz);
void commaPrint0dp(FILE *fp, double d);
int getWriteCache(const char *devicesuffix);
int splitRange(const char *charBS, double *low, double *high);
char *getModel(const char *suffix);
size_t freeRAM();
size_t canCreateFile(const char *filename, const size_t sz);
char *hostname();
size_t getUptime();

#endif

