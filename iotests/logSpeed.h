#ifndef _LOGSPEED_H
#define _LOGSPEED_H

#include <unistd.h>

typedef struct {
  double starttime;
  size_t num;
  size_t alloc;
  double *values;
  double lasttime;
} logSpeedType;

void logSpeedInit(volatile logSpeedType *l);
int logSpeedAdd(logSpeedType *l, size_t value);
double logSpeedTime(logSpeedType *l);
double logSpeedMedian(logSpeedType *l);
void logSpeedFree(logSpeedType *l);
size_t logSpeedN(logSpeedType *l);
void logSpeedReset(logSpeedType *l);


#endif

