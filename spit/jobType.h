#ifndef _JOBTYPE_H
#define _JOBTYPE_H

#include <stdlib.h>
#include "devices.h"

typedef struct {
  int count;
  char **strings;
  char **devices;
} jobType;
 

void jobInit(jobType *j);
void jobAdd(jobType *j, const char *jobstring);
void jobDump(jobType *j);
void jobFree(jobType *j);
size_t jobCount(jobType *j);
void jobRunThreads(jobType *job, const int num, const size_t maxSizeInBytes, const size_t timetorun, const size_t dumpPos, char *benchmarkName, const size_t origqd);
void jobMultiply(jobType *job, const size_t extrajobs, deviceDetails *deviceList, size_t deviceCount);
void jobAddDeviceToAll(jobType *j, const char *device);

#endif

