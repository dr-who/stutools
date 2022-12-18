#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <getopt.h>

#include "histogram.h"
#include "numList.h"

int main(int argc, char *argv[])
{
  
  double minValue = 0;
  double maxValue = 1000;
  double binSize = 0.01;
  char *prefix = "histogram";
  double incomingScale = 1;
  size_t dumpData = 0;
  char *inputFilename = NULL;


  int opt;
  const char *getoptstring = "m:M:b:o:s:di:";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'i':
      inputFilename = strdup(optarg);
      break;
    case 'm':
      minValue = atof(optarg);
      break;
    case 'M':
      maxValue = atof(optarg);
      break;
    case 'd':
      dumpData = 1;
      break;
    case 's':
      incomingScale = atof(optarg);
      break;
    case 'b':
      binSize = atof(optarg);
      if (binSize == 0) binSize = 0.01;
      break;
    case 'o':
      prefix = strdup(optarg);
      break;
    default:
      fprintf(stderr,"*error* unknown option '%s'\n", optarg);
      exit(-1);
    }
  }

  char datafile[1000], gnufile[1000], pngfile[1000];
  sprintf(datafile, "%s-bins.txt", prefix);
  sprintf(gnufile, "%s.gnu", prefix);
  sprintf(pngfile, "%s.png", prefix);
  fprintf(stderr,"*info* hist: [%g, %g] binSize %g, file '%s', scale %g, prefix '%s'\n", minValue, maxValue, binSize, inputFilename ? inputFilename : "<stdin>", incomingScale, prefix);

  
  histogramType h;

  histSetup(&h, minValue, maxValue, binSize);
  srand48(0);

  //  for (size_t i = 0; i < 100000; i++) {
  //    histAdd(&h, drand48());
  //  }

  //  for (size_t i = 0; i < 100000; i++) {
  //    histAdd(&h, drand48()/2);
  //  }

  numListType l;
  nlInit(&l, 10000000);


  FILE *fin = stdin;
  if (inputFilename) {
    fin = fopen(inputFilename, "rt");
    if (!fin) {
      perror(inputFilename);
      exit(1);
    }
  }
  double v = 0;
  while (fscanf(fin, "%lf", &v) == 1) {
    v = v * incomingScale;
    histAdd(&h, v);
    nlAdd(&l, v);

    if (dumpData) {
      fprintf(stdout,"%lf\n", v);
    }
  }
  if (inputFilename) {
    fclose(fin);
  }


  double median, three9, four9, five9;
  histSumPercentages(&h, &median, &three9, &four9, &five9, 1);
  histSave(&h, datafile, 1);
  char xlabel[100];
  sprintf(xlabel, "Time (ms) - %g bins", binSize);
  histWriteGnuplot(&h, datafile, gnufile, pngfile, xlabel, "Count");

  fprintf(stderr,"*info* raw: mean %g\n", nlMean(&l));
  fprintf(stderr,"*info* raw: median %g, 99.9%% %lf, 99.99%% %lf, 99.999%% %lf\n",  nlMedian(&l), nlSortedPos(&l, 0.999), nlSortedPos(&l, 0.9999), nlSortedPos(&l, 0.99999));
  fprintf(stderr,"*info* bin: median %lf, 99.9%% %lf, 99.99%% %lf, 99.999%% %lf\n", median, three9, four9, five9);
  const double consistency = histConsistency(&h);
  fprintf(stderr,"*info* consistency score %.1lf%% using binSize of %g\n", consistency, binSize);

  nlFree(&l);
	  
  histFree(&h);
  return 0;
}
