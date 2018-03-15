#ifndef _LOGSPEED_H
#define _LOGSPEED_H

#include <unistd.h>

#define OUTPUTINTERVAL 1

#define JSON 1
#define MYSQL 2

typedef struct {
  double starttime;
  size_t num;
  size_t alloc;
  double *rawtime;
  double *rawvalues;
  size_t *rawcount;
  double lasttime;
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

void   logSpeedDump(logSpeedType *l, const char *fn, const int format, const char *description, size_t bdSize, size_t origBdSize, const char *cli);
void logSpeedHistogram(logSpeedType *l);
void logSpeedCheckpoint(logSpeedType *l);
double logSpeedGetCheckpoint(logSpeedType *l);


#endif

