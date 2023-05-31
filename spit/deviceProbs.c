#define _XOPEN_SOURCE 500

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <assert.h>
#include <string.h>
#include <math.h>

#include "deviceProbs.h"


void deviceProbInit(deviceProbType *dp, const size_t maxdays) {
  dp->probs = NULL;
  dp->maxdays = maxdays;
}

void deviceProbFlat(deviceProbType *dp, const float yearlysurviverate) {

  dp->probs = calloc(dp->maxdays, sizeof(float));
  for (size_t i = 0; i < dp->maxdays; i++) {
    dp->probs[i] = (1.0 - (pow(yearlysurviverate, 1.0 / 365))); // daily prob of failure
    //    if (i==0) {
    //      fprintf(stderr," %lf becomes %lf\n", yearlysurviverate, dp->probs[i]);
    //    }
  }
}

void deviceProbLoad(deviceProbType *dp, const char *fn, const int verbose) {

  FILE *fp = fopen(fn, "rt");
  float a,b,c;
  if (fp) {
    if (verbose) fprintf(stderr,"*info* opening survival rate file '%s'\n", fn);
    while (fscanf(fp, "%f %f %f\n", &a, &b, &c) > 0) {
      if (verbose) fprintf(stderr,"%f %f %f\n", a,b,c);
      for (size_t i = a; i < b; i++) {
	if (i < dp->maxdays) 
	  dp->probs[i] = 1.0 - pow(1.0-c, 1/365.0);
      }
    }
    for (size_t i = b; i < dp->maxdays; i++) {
      if (i < dp->maxdays) 
	dp->probs[i] = 1.0 - pow(1.0-c, 1/365.0);
    }
    fclose(fp);
  } else {
    perror(fn);
    exit(1);
  }

  for (size_t i = 0; i < dp->maxdays; i++) {
    if (dp->probs[i] == 0) {
      abort();
    }
  }

  if (verbose) fprintf(stderr,"The last day has a (yearly) survival rate of %.2lf\n", dp->probs[dp->maxdays-1]);
}

void deviceProbDump(deviceProbType *dp, const char *fn) {
  FILE *fp = fopen(fn, "wt");
  if (fp) {
    for (size_t i = 0; i < dp->maxdays; i++) {
      fprintf(fp, "%zd\t%f\n", i, dp->probs[i]);
    }
    fclose(fp);
  } else {
    perror(fn);
    exit(1);
  }
}

