#ifndef _BLOCKDEVICE_H
#define _BLOCKDEVICE_H

#include <stdio.h>

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
void bdDumpKV(FILE *fp, bdType *bd);

#endif
