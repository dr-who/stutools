#ifndef _FAKECLUSTER_H
#define _FAKECLUSTER_H


typedef struct {
  int id;
  double time;
  int RAMGB;
  int numGPUs;
  int numDevices;
  int bdSize;
} fakeclusterType;

fakeclusterType *fakeclusterInit(int size);

#endif
