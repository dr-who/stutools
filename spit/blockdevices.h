#ifndef __BLOCKDEVICES_H
#define __BLOCKDEVICES_H



#include "keyvalue.h"

typedef struct {
  keyvalueType *kv;
} blockDeviceType;

typedef struct {
  size_t num;

  blockDeviceType *devices;
} blockDevicesType;



blockDevicesType * blockDevicesInit();

void blockDevicesScan(blockDevicesType *bd);

void blockDevicesFree(blockDevicesType *bd);

char * blockDevicesAllJSON(blockDevicesType *bd);

size_t blockDevicesCount(blockDevicesType *bd, const char *s, size_t *sumbytes); // SSD, HDD type


#endif
