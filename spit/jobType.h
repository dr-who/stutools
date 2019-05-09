#ifndef _JOBTYPE_H
#define _JOBTYPE_H

#include <stdlib.h>

typedef struct {
  int count;
  char **strings;
  char **devices;
} jobType;
 

void jobInit(jobType *j);
void jobAdd(jobType *j, const char *jobstring);
void jobDump(jobType *j);
void jobFree(jobType *j);
void jobRunThreads(jobType *j, const int num, const size_t maxSizeInBytes, const size_t timetorun, const size_t dumpPositions, char *benchmarkName);
void jobMultiply(jobType *j, const size_t extrajobs);
void jobAddDeviceToAll(jobType *j, const char *device);

#endif

