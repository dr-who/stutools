#ifndef _BLOCKDEVICE_H
#define _BLOCKDEVICE_H

#include <stdio.h>
#include "json.h"

typedef struct {
  char *path;
  unsigned int major;
  unsigned int minor;
  long long size;
  int mapper;
  int bs;
  int rotational;
  int removable;
  int zoned;
  int volatil;
  int partition;
  char *vendor;
  char *model;
  char *serial;
  char *usbdriver;
  char *writecache;

  char *type;// overall
} bdType;


bdType *bdInit(const char *path);
void bdFree(bdType **bd);

int majorAndMinor(int fd, unsigned int *major, unsigned int *minor);
jsonValue *bdJsonValue(bdType *bd);

jsonValue * bdScan(void);



#endif
