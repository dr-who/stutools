#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <getopt.h>

#include "histogram.h"
#include "numList.h"

int main(int argc, char *argv[])
{
  char *histfile = "hist.out";
  size_t samples = 10;

  size_t calcMin = 0, calcMax = 0, calcMean = 0;
  
  int opt;
  const char *getoptstring = "i:s:mMa";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'a':
      calcMean = 1;
      break;
    case 'i':
      histfile = strdup(optarg);
      break;
    case 's':
      samples = atoi(optarg);
      break;
    case 'm':
      calcMin = 1;
      break;
    case 'M':
      calcMax = 1;
      break;
    default:
      fprintf(stderr,"*error* unknown option '%s'\n", optarg);
      exit(-1);
    }
  }

  histogramType h;
  histLoad(&h, histfile);

  const double consistency = histConsistency(&h);
  fprintf(stderr,"*info* consistency score %.1lf%% using binSize of %zd\n", consistency, h.binScale);

  srand48(getDevRandom());

  double theMin = NAN, theMax = NAN, theSum = NAN;
  for (size_t i = 0; i < samples; i++) {
    const double sample = histSample(&h);
    fprintf(stdout, "%lf\n", sample);
    if (i==0) {
      theMin = sample;
      theMax = sample;
      theSum = sample;
    } else {
      if (sample < theMin) theMin = sample;
      if (sample > theMax) theMax = sample;
      theSum += sample;
    }
  }

  histFree(&h);
  if (calcMin) {
    fprintf(stderr,"*info* min value %lf\n", theMin);
  }
  if (calcMean) {
    fprintf(stderr,"*info* mean value %lf\n", theSum / samples);
  }
  if (calcMax) {
    fprintf(stderr,"*info* max value %lf\n", theMax);
  }
  return 0;
}
