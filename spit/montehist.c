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

  int opt;
  const char *getoptstring = "i:s:";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'i':
      histfile = strdup(optarg);
      break;
    case 's':
      samples = atoi(optarg);
      break;
    default:
      fprintf(stderr,"*error* unknown option '%s'\n", optarg);
      exit(-1);
    }
  }

  histogramType h;
  histLoad(&h, histfile);

  const double consistency = histConsistency(&h);
  fprintf(stderr,"*info* consistency score %.1lf%% using binSize of %g\n", consistency, 1.0 / h.binScale);

  srand48(getDevRandom());
  for (size_t i = 0; i < samples; i++) {
    fprintf(stdout, "%lf\n", histSample(&h));
  }
	  
  histFree(&h);
  return 0;
}
