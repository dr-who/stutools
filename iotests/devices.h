#ifndef _DEVICES_H
#define _DEVICES_H


typedef struct {
  int fd;
  size_t bdSize;
  char *devicename;
} deviceDetails;


void addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
void freeDeviceDetails(deviceDetails *devs, size_t numDevs);
size_t loadDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs);
void openDevices(deviceDetails *devs, size_t numDevs, size_t maxGiBOverride);

#endif

