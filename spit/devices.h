#ifndef _DEVICES_H
#define _DEVICES_H

#define _GNU_SOURCE_H

#include <libaio.h>

typedef struct {
  int fd;
  size_t bdSize;
  size_t shouldBeSize;
  char *devicename;
  int exclusive;
  int isBD;
  int ctxIndex;
  int numa;
} deviceDetails;

// a set of functions to keep io contexts together
io_context_t *createContexts(const size_t count, const size_t qd);
void setupContexts(io_context_t *p, const size_t count, const size_t qd);
void freeContexts(io_context_t *p, const size_t count);

deviceDetails * addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
deviceDetails *prune(deviceDetails *devList, size_t *devCount, const size_t blockSize);
void freeDeviceDetails(deviceDetails *devs, size_t numDevs);
size_t loadDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);

void openDevices(deviceDetails *devs, size_t numDevs, const size_t sendTrim, size_t *maxSizeInBytes, size_t LOWBLKSIZE, size_t BLKSIZE, size_t alignment, int needToWrite, int dontUseExclusive, const size_t contextCount);

void infoDevices(const deviceDetails *devList, const size_t devCount);

size_t numOpenDevices(deviceDetails *devs, size_t numDevs);
size_t smallestBDSize(deviceDetails *devList, size_t devCount);
size_t expandDevices(deviceDetails **devs, size_t *numDevs, int *seqFiles, double *maxSizeGiB);

size_t getIOPSestimate(const char *fn, const size_t blocksize, const int verbose);

#endif
