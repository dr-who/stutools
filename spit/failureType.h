#ifndef __FAILURETYPE_H
#define __FAILURETYPE _H

#include "deviceProbs.h"

typedef struct {
  float *probs;
  size_t probLen;
  float flatProb;

  size_t k;
  size_t m;
  size_t LHSmax;
  size_t LHS;
  int LHSstatus;
  size_t age;
  size_t agesincerebuilt;
  size_t driveSizeInBytes;

  size_t drivesFailed;

  int *status;
  size_t total;

  char *name;

  deviceProbType dp;
  
} virtualDeviceType;


typedef struct {
  virtualDeviceType *f;
  size_t len;
  // global options
  size_t GHS;
  size_t order;
  size_t rebuildSpeedInBytesPerSec;
} failureGroup;

int failureType(int argc, char *argv[]);

#endif

