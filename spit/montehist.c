#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <getopt.h>

#include "histogram.h"
#include "numList.h"
#include "utils.h"

int keepRunning = 1;

void usage() {
  fprintf(stdout,"Usage:\n");
  fprintf(stdout,"  montehist -i hist [-s n] [-m -M -a] [-r n]\n");
  fprintf(stdout,"\n");
  fprintf(stdout,"Options:\n");
  fprintf(stdout,"  -n n    # specify the number of samples\n");
  fprintf(stdout,"  -r n    # perform <n> runs of <sn> samples, as a table\n");
  fprintf(stdout,"  -R      # performing a rolling row based sum per sample\n");
  fprintf(stdout,"  -s n    # scale up the input by * n\n");
}

int main(int argc, char *argv[])
{
  char *histfile = "hist.out";
  size_t samples = 10;
  size_t rows = 0;
  double scale = 1.0;
  size_t rollingsum = 0;
  
  int opt;
  const char *getoptstring = "i:s:Rr:hn:";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
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
    case 'R':
      rollingsum = 1;
      break;
    case 's':
      scale = atof(optarg);
      break;
    case 'n':
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
  if (samples) fprintf(stderr,"*info* samples = %zd\n", samples);
  if (rows) fprintf(stderr,"*info* rows = %zd\n", rows);
  fprintf(stderr,"*info* consistency score %.1lf%% using binSize of %zd\n", consistency, h.binScale);

  srand48(getDevRandom());

  size_t r = 0;
  
  double *samplearray = calloc(samples, sizeof(double)); assert(samplearray);

  do {
    r++;
    
    numListType nl;
    nlInit(&nl, samples);


    for (size_t i = 0; i < samples; i++) {
      if (rollingsum) { 
	samplearray[i] += histSample(&h) * scale;
      } else {
	samplearray[i] = histSample(&h) * scale;
      }
    }

    for (size_t i = 0; i < samples; i++) {
      if (rows == 0) {
	fprintf(stdout, "%lf\n", samplearray[i]);
      }
      nlAdd(&nl, samplearray[i]);
    }
    
    if (rows == 0) {
      fprintf(stderr, "*info* N values %zd\n", nlN(&nl));
      fprintf(stderr, "*info* min value %lf\n", nlMin(&nl));
      fprintf(stderr, "*info* mean value %lf\n", nlMean(&nl));
      fprintf(stderr, "*info* max value %lf\n", nlMax(&nl));
      fprintf(stderr, "*info* SD value %lf\n", nlSD(&nl));
    } else {
      fprintf(stdout, "%zd\t%lf\t%lf\t%lf\t%lf\n", r, nlMin(&nl), nlMean(&nl), nlMax(&nl), nlSD(&nl));
    }
    nlFree(&nl);
  } while (r < rows);
  free(samplearray);

  histFree(&h);
  return 0;
}
