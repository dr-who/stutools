#ifndef _LATENCY_H
#define _LATENCY_H

#include "histogram.h"
#include "positions.h"

typedef struct {
  histogramType histRead;
  histogramType histWrite;

} latencyType;

void latencySetup(latencyType *lat, positionContainer *pc);

void latencyStats(latencyType *lat);

void latencyFree(latencyType *lat);

void latencyWriteGnuplot(latencyType *lat);

void latencyReadGnuplot(latencyType *lat);

#endif
