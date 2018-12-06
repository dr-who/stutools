#ifndef _JOBTYPE_H
#define _JOBTYPE_H

#include <stdlib.h>
#include "devices.h"

typedef struct {
  int count;
  char **strings;
} jobType;
 

void jobInit(jobType *j);
void jobAdd(jobType *j, char *str);
void jobDump(jobType *j);
void jobFree(jobType *j);
void jobRunThreads(jobType *j, const int num, const size_t maxSizeInBytes, const size_t lowbs, deviceDetails *dev, size_t timetorun);

#endif

