#ifndef _UTILS_H
#define _UTILS_H

#include <malloc.h>
#include <string.h>
#include <stdio.h> // FILE
#include <pthread.h>
#include <assert.h>

#include "logSpeed.h"

// https://www.openwall.com/lists/musl/2020/01/20/4
#ifndef __GLIBC__
#define ioctl(fd, req, ...) ioctl(fd, (int)(req), ##__VA_ARGS__)
#endif

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

#define GBtoGiB(x)  (((x) * 1000.0) / 1024.0)



/*#define CALLOC(x, y, z) {x = calloc(y, z); if (!(x)) {fprintf(stderr,"ooom!!\n");abort();}}*/
#define CALLOC(x, y, z) {/*fprintf(stderr,"CALLOC %s %d, size %zd\n", __FILE__, __LINE__, ((((size_t)((y) * (z)))/4096 + 1) * 4096));*/ assert((y)>0); assert((z)>0); x = memalign(4096, (((size_t)((y) * (z)))/4096 + 1) * 4096); if(x) memset(x, 0, (((size_t)((y) * (z)))/4096 + 1) * 4096);  if (!(x)) {fprintf(stderr,"*error* out of memory! ooom!! %zd\n", (((size_t)((y) * (z)))/4096 + 1) * 4096);abort();}}

#define DIFF(x,y) ((x) > (y)) ? ((x)-(y)) : ((y) - (x))

double timedouble();

void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess, int verifyWrites, float flushEverySecs);
void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess);

size_t blockDeviceSize(const char *name);
int isBlockDevice(const char *name);
size_t blockDeviceSizeFromFD(const int fd);


void dropCaches();
void printPowerMode();
char *username();
void usernameinit();

char* queueType(char *path);

void checkContents(char *label, char *charbuf, size_t size, const size_t checksum, float checkpercentage, size_t stopatbytes);
void shmemUnlink();

double loadAverage();
size_t numThreads();
size_t totalRAM();
size_t totalShared();
size_t totalBuffer();
char *OSRelease();
size_t swapTotal();

size_t fileSize(int fd);
size_t fileSizeFromName(const char *path);
size_t fileExists(const char *path);

void majorAndMinor(int fd, unsigned int *major, unsigned int *minor);

int getWriteCacheStatus(int fd);
int trimDevice(int fd, char *path, unsigned long low, unsigned long high);

void checkRandomBuffer4k(const char *buffer, const size_t size);
size_t generateRandomBuffer(char *buffer, const size_t size, unsigned short seed);
size_t generateRandomBufferCyclic(char *buffer, const size_t size, unsigned short seed, size_t cyclic);
size_t checksumBuffer(const char *buffer, const size_t size);


char *getSuffix(const char *path);
char *getScheduler(const char *path);

size_t alignedNumber(size_t num, size_t alignment);
size_t randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits, const size_t randomValue);
int getNumRequests(const char *suf);



//void getPhyLogSizes(const char *suffix, size_t *phy, size_t *log);
void getPhyLogSizes(const char *suffix, size_t *phy, size_t *max_io_bytes, size_t *log);

int startsWith(const char *pre, const char *str);
int canOpenExclusively(const char *fn);
int createFile(const char *filename, const size_t sz);
void commaPrint0dp(FILE *fp, double d);
int getWriteCache(const char *devicesuffix);
int splitRange(const char *charBS, double *low, double *high);
int splitRangeChar(const char *charBS, double *low, double *high, char *retch);

char *getCPUModel();
char *getModel(const char *suffix);
size_t freeRAM();
size_t canCreateFile(const char *filename, const size_t sz);
char *hostname();
size_t getUptime();
int getRotational(const char *suf);

size_t dirtyPagesBytes();
size_t getCachedBytes();

int runCommand(char *program, char *argv_list[]);

int getNumaCount();
int getNumHardwareThreads();
int cpuCountPerNuma( int numa );
void getThreadIDs( int numa, int* numa_cpu_list );
int pinThread( pthread_t* thread, int* hw_tids, size_t n_hw_tid );

void getBaseBlockDevice(const char *block_device, char* base_block_device);
int getDiscardInfo(const char *suffix, size_t *alignment_offset, size_t *discard_max_bytes, size_t *discard_granularity, size_t *discard_zeroes_data);
int performDiscard(int fd, const char *path, unsigned long low, unsigned long high, size_t max_bytes, size_t discard_granularity, double *maxdelay_secs, const int verbose, int zeroall);
void syslogString(const char *prog, const char *message);
size_t stringToBytesDefaultGiB(const char *str);

#endif

