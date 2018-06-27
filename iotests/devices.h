#ifndef _DEVICES_H
#define _DEVICES_H


typedef struct {
  int fd;
  size_t bdSize;
  char *devicename;
  int exclusive;
  double maxSizeGiB;
  int isBD;
} deviceDetails;


deviceDetails * addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
void freeDeviceDetails(deviceDetails *devs, size_t numDevs);
size_t loadDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
size_t openDevices(deviceDetails *devs, size_t numDevs, const size_t sendTrim, double maxSizeGB, size_t LOWBLKSIZE, size_t BLKSIZE, size_t alignment, int needToWrite, int dontUseExclusive);
void infoDevices(const deviceDetails *devList, const size_t devCount);

size_t numOpenDevices(deviceDetails *devs, size_t numDevs);


#endif
