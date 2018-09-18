#ifndef _DEVICES_H
#define _DEVICES_H


typedef struct {
  int fd;
  size_t bdSize;
  size_t shouldBeSize;
  char *devicename;
  int exclusive;
  int isBD;
} deviceDetails;


deviceDetails * addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
deviceDetails *prune(deviceDetails *devList, size_t *devCount, const size_t blockSize);
void freeDeviceDetails(deviceDetails *devs, size_t numDevs);
size_t loadDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
void openDevices(deviceDetails *devs, size_t numDevs, const size_t sendTrim, size_t *maxSizeInBytes, size_t LOWBLKSIZE, size_t BLKSIZE, size_t alignment, int needToWrite, int dontUseExclusive);
void infoDevices(const deviceDetails *devList, const size_t devCount);

size_t numOpenDevices(deviceDetails *devs, size_t numDevs);
size_t smallestBDSize(deviceDetails *devList, size_t devCount);
size_t expandDevices(deviceDetails **devs, size_t *numDevs, int *seqFiles, double *maxSizeGiB);


#endif
