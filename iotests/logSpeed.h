#ifndef _LOGSPEED_H
#define _LOGSPEED_H

#include <unistd.h>

typedef struct {
  double starttime;
  size_t num;
  size_t alloc;
  size_t total;
  double *values;
  double lasttime;
} logSpeedType;

void logSpeedInit(volatile logSpeedType *l);
int logSpeedAdd(logSpeedType *l, size_t value);
double logSpeedTime(logSpeedType *l);
double logSpeedMedian(logSpeedType *l);
double logSpeedMean(logSpeedType *l);
void logSpeedFree(logSpeedType *l);
size_t logSpeedN(logSpeedType *l);
void logSpeedReset(logSpeedType *l);
size_t logSpeedTotal(logSpeedType *l);
double logSpeed5(logSpeedType *l);
double logSpeed95(logSpeedType *l);
double logSpeed99(logSpeedType *l);
double logSpeedMax(logSpeedType *l);
void logSpeedDump(logSpeedType *l, const char *fn);


#endif

