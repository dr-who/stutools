#ifndef _LOGSPEED_H
#define _LOGSPEED_H

#include <unistd.h>

#define OUTPUTINTERVAL 1

#define JSON 1

typedef struct {
  double starttime;
  size_t num;
  size_t alloc;
  double total;
  double rawtot;
  double checkpoint;
  double *rawtime;
  double *rawvalues;
  size_t *rawcount;
  double *values;
  double lasttime;
  int    sorted;
} logSpeedType;

double logSpeedInit(volatile logSpeedType *l);
void   logSpeedFree(logSpeedType *l);
void   logSpeedReset(logSpeedType *l);

int    logSpeedAdd(logSpeedType *l, double value);
int    logSpeedAdd2(logSpeedType *l, double value, size_t count);

double logSpeedTime(logSpeedType *l);

double logSpeedMedian(logSpeedType *l);
double logSpeedMean(logSpeedType *l);
size_t logSpeedN(logSpeedType *l);
double logSpeedTotal(logSpeedType *l);
double logSpeedRank(logSpeedType *l, float rank); // between [0...1)
double logSpeedMax(logSpeedType *l);

void   logSpeedDump(logSpeedType *l, const char *fn, const int format);
void logSpeedHistogram(logSpeedType *l);
void logSpeedCheckpoint(logSpeedType *l);
double logSpeedGetCheckpoint(logSpeedType *l);


#endif

