#ifndef _DEVICE_PROBS_H
#define _DEVICE_PROBS_H


float *setupProbsFlat(int maxdays, float prob);

float *setupProbs(char *fn, int maxdays, int verbose);

void dumpProbs(const char *fn, const float *days, const int maxdays);


#endif


