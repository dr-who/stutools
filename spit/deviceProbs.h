#ifndef _DEVICE_PROBS_H
#define _DEVICE_PROBS_H

#include <stdlib.h>

typedef struct {
  float *probs;
  size_t maxdays;
} deviceProbType;


void deviceProbInit(deviceProbType *dp, const size_t maxdays);

void deviceProbFlat(deviceProbType *dp, const float prob);

void deviceProbLoad(deviceProbType *dp, const char *filename, const int verbose);

void deviceProbDump(deviceProbType *dp, const char *dumpname);

#endif


