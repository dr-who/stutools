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
  int *suggestedNUMA;
} jobType;


typedef struct {
  double rprob;
  double wprob;
  double tprob;
} probType;

typedef struct {
  double readIOPS;
  double writeIOPS;
  double readBps;
  double writeBps;
  size_t readTotalIO;
  size_t writeTotalIO;
} resultType;

void jobInit(jobType *j);
void jobAdd(jobType *j, const char *jobstring);
void jobAddExec(jobType *j, const char *jobstring, const double delay);
void jobDump(jobType *j);
void jobFree(jobType *j);
size_t jobCount(jobType *j);

void jobRunThreads(jobType *job, const int num, char *filePrefix,
                   const size_t minSizeInBytes,
                   const size_t maxSizeInBytes,
                   const double runseconds, const size_t dumpPos, char *benchmarkName, const size_t origqd,
                   unsigned short seed, FILE *savePositions, diskStatType *d, const double timeperline, const double ignorefirst, const size_t verify,
                   char *mysqloptions, char *mysqloptions2, char *commandstring, char* numaBinding, const int performDiscard, resultType *result, size_t ramBytesForPositions, size_t notexclusive, const size_t showdate);

void jobMultiply(jobType *retjobs, jobType *job, deviceDetails *deviceList, size_t deviceCount);

//  void jobMultiply(jobType *job, const size_t extrajobs, deviceDetails *deviceList, size_t deviceCount);
void jobAddDeviceToAll(jobType *j, const char *device);
void jobAddBoth(jobType *job, char *device, char *jobstring, int suggestNUMA);
void jobFileSequence(jobType *job);
size_t jobRunPreconditions(jobType *preconditions, const size_t count, const size_t minSizeBytes, const size_t maxSizeBytes);

void resultDump(const resultType *r, const char *kcheckresult, const int display);

#endif

