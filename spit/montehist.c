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

  fprintf(stdout,"  montehist -i hist [-s n] [-m -M -a] [-r n] [options]\n");
  fprintf(stdout,"\n");
  fprintf(stdout,"Options:\n");
  fprintf(stdout,"  -a            # print as atomic 'double' or 'size_t' types\n");
  fprintf(stdout,"  -i hist.txt   # load distribution from a histogram file\n"); 
  fprintf(stdout,"  -l l,h,a      # linear distribution between [l, h), alignment 'a'\n");
  fprintf(stdout,"  -n n          # specify the number of samples\n");
  fprintf(stdout,"  -r n          # perform <n> runs of <sn> samples, as a table\n");
  fprintf(stdout,"  -R            # performing a rolling row based sum per sample\n");
  fprintf(stdout,"  -s n          # scale up the values by * n\n");
  fprintf(stdout,"  -S n          # scale down the values by / n\n");
  fprintf(stdout,"\nExamples:\n");
  fprintf(stdout,"  montehist -l 0,3PB,4096          # generate random 4KiB aligned positions\n");
  fprintf(stdout,"  montehist -l 0,3PB,4096  -S4096  # / 4096, effectively giving a unique index\n");
}

int main(int argc, char *argv[])
{
  char *histfile = NULL;
  size_t samples = 10;
  size_t rows = 0;
  double scale = 1.0;
  size_t rollingsum = 0;
  size_t linear = 0; // if not linear then load from histogram file
  size_t low = 0, high = 0, alignment = 0;
  size_t atomic = 0;
  
  int opt;
  const char *getoptstring = "ai:s:S:Rr:hn:l:";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'a':
      atomic = 1;
      break;
    case 'h':
      usage();
      exit(1);
      break;
    case 'i':
      histfile = strdup(optarg);
      break;
    case 'l':
      {
      char *first = strtok(optarg, ",");
      if (first) {
	char *second = strtok(NULL, ",");
	if (second) {
	  char *third = strtok(NULL, ",");
	  if (third) {
	    alignment = atoi(third);
	  }
	  linear = 1;
	  low = stringToBytesDefault(first, 1, 1);
	  high = stringToBytesDefault(second, 1, 1);
	  fprintf(stderr, "*info* linear distribution in range [%zd, %zd), alignment %zd\n", low, high, alignment);
	}
      }}
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
    case 'S':
      scale = 1.0 / atof(optarg);
      break;
    case 'n':
      samples = atoi(optarg);
      break;
    default:
      fprintf(stderr,"*error* unknown option '%s'\n", optarg);
      exit(-1);
    }
  }

  if (histfile==NULL && linear==0) {
    usage();
    exit(-1);
  }

  histogramType h;

  if (linear == 0) {
    histLoad(&h, histfile);
    const double consistency = histConsistency(&h);
    fprintf(stderr,"*info* consistency score %.1lf%% using binSize of %zd\n", consistency, h.binScale);
  }

  if (samples) fprintf(stderr,"*info* samples = %zd\n", samples);
  if (rows) fprintf(stderr,"*info* rows = %zd\n", rows);

  srand48(getDevRandom());

  size_t r = 0;
  
  double *samplearray = calloc(samples, sizeof(double)); assert(samplearray);

  do {
    r++;
    
    numListType nl;
    nlInit(&nl, samples);


    for (size_t i = 0; i < samples; i++) {
      double value = NAN;
      if (linear == 0) {
	value = histSample(&h);
      } else {
	//	fprintf(stderr,"%zd\n", (high - low));
	value = randomBlockSize(low, high, log(alignment*1.0)/log(2), (size_t)(drand48() * (high - low)));
      }
      value = value * scale;

      if (rollingsum) { 
	samplearray[i] += value;
      } else {
	samplearray[i] = value;
      }
    }

    for (size_t i = 0; i < samples; i++) {
      if (rows == 0) {
	if (atomic) {
	  if (alignment * scale == 1) {
	    size_t v = samplearray[i];
	    int ret = write(STDOUT_FILENO, &v, sizeof(size_t));
	    if (ret < 8) {
	      perror("eek");
	    }
	  } else {
	    int ret = write(STDOUT_FILENO, &samplearray[i], sizeof(double));
	    if (ret < 8) {
	      perror("eek");
	    }
	  }
	} else {
	  fprintf(stdout, "%lf\n", samplearray[i]);
	}
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

  if (linear == 0) {
    histFree(&h);
  }
  return 0;
}
