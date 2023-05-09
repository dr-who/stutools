#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <getopt.h>

#include "histogram.h"
#include "numList.h"

int keepRunning = 1;

void usage() {
  fprintf(stdout,"Usage:\n");
  fprintf(stdout,"  montehist -i hist [-s n] [-m -M -a] [-r n]\n");
  fprintf(stdout,"\n");
  fprintf(stdout,"Options:\n");
  fprintf(stdout,"  -s sn    # specify the number of samples\n");
  fprintf(stdout,"  -m       # output the minimum of the <sn> samples\n");
  fprintf(stdout,"  -a       # output the mean of the <sn> samples\n");
  fprintf(stdout,"  -M       # output the max of the <sn> samples\n");
  fprintf(stdout,"  -r n     # perform <n> runs of <sn> samples, as a table\n");
}

int main(int argc, char *argv[])
{
  char *histfile = "hist.out";
  size_t samples = 10;

  size_t calcMin = 0, calcMax = 0, calcMean = 0;

  size_t rows = 0;
  
  int opt;
  const char *getoptstring = "i:s:mMar:h";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'a':
      calcMean = 1;
      break;
    case 'h':
      usage();
      exit(1);
      break;
    case 'i':
      histfile = strdup(optarg);
      break;
    case 'r':
      rows = atoi(optarg); 
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
  if (samples) fprintf(stderr,"*info* samples = %zd\n", samples);
  if (rows) fprintf(stderr,"*info* rows = %zd\n", rows);
  fprintf(stderr,"*info* consistency score %.1lf%% using binSize of %zd\n", consistency, h.binScale);

  srand48(getDevRandom());

  size_t r = 0;
  
  do {
    r++;
    
    double theMin = NAN, theMax = NAN, theSum = NAN;
    for (size_t i = 0; i < samples; i++) {
      const double sample = histSample(&h);
      if (rows == 0) {
	fprintf(stdout, "%lf\n", sample);
      }
      
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

    if (rows == 0) {
      if (calcMin) {
	fprintf(stderr,"*info* min value %lf\n", theMin);
      }
      if (calcMean) {
	fprintf(stderr,"*info* mean value %lf\n", theSum / samples);
      }
      if (calcMax) {
	fprintf(stderr,"*info* max value %lf\n", theMax);
      }
    } else {
      fprintf(stdout,"%zd\t%lf\t%lf\t%lf\n", r, theMin, theSum/samples, theMax);
    }
    
  } while (r < rows);

  histFree(&h);
  return 0;
}
