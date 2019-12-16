#ifndef _JOBTYPE_H
#define _JOBTYPE_H

#include <stdlib.h>
#include "devices.h"
#include "diskStats.h"

typedef struct {
  int count;
  char **strings;
  char **devices;
  int *deviceid;
  double *delay;
} jobType;
 

void jobInit(jobType *j);
void jobAdd(jobType *j, const char *jobstring);
void jobAddExec(jobType *j, const char *jobstring, const double delay);
void jobDump(jobType *j);
void jobFree(jobType *j);
size_t jobCount(jobType *j);

void jobRunThreads(jobType *job, const int num, char *filePrefix, 
		   const size_t minSizeInBytes,
		   const size_t maxSizeInBytes,
		   const size_t timetorun, const size_t dumpPos, char *benchmarkName, const size_t origqd,
		   unsigned short seed, const char *savePositions, diskStatType *d, const double timeperline, const double ignorefirst, const size_t verify,
		   char *mysqloptions, char *mysqloptions2, char *commandstring);

void jobMultiply(jobType *job, const size_t extrajobs, deviceDetails *deviceList, size_t deviceCount);
void jobAddDeviceToAll(jobType *j, const char *device);
void jobAddBoth(jobType *job, char *device, char *jobstring);
void jobFileSequence(jobType *job);
size_t jobRunPreconditions(jobType *preconditions, const size_t count, const size_t minSizeBytes, const size_t maxSizeBytes);

#endif

