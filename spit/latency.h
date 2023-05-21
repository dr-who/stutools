#ifndef _LATENCY_H
#define _LATENCY_H

#include "histogram.h"
#include "positions.h"

typedef struct {
    histogramType histRead;
    histogramType histWrite;

} latencyType;

void latencyClear(latencyType *lat);

void latencySetup(latencyType *lat, positionContainer *pc);

void latencySetupSizeonly(latencyType *lat, positionContainer *pc, size_t size);

void latencyStats(latencyType *lat);

void latencyFree(latencyType *lat);

void latencyWriteGnuplot(latencyType *lat);

void latencyReadGnuplot(latencyType *lat);

void latencyLenVsLatency(positionContainer *origpc, int num);

void latencyOverTime(positionContainer *origpc);


#endif
