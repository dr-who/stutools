#define _XOPEN_SOURCE
#include <stdlib.h>
#include <stdio.h>

#include "histogram.h"

int main() {
  histogramType h;

  histSetup(&h);
  srand48(0);

  for (size_t i = 0; i < 100000; i++) {
    histAdd(&h, drand48());
  }

  for (size_t i = 0; i < 100000; i++) {
    histAdd(&h, drand48()/2);
  }


  fprintf(stderr,"%lf\n", histMean(&h));


  double median, three9, four9, five9;
  histSumPercentages(&h, &median, &three9, &four9, &five9, 1);
  histSave(&h, "hist.out", 1);

  fprintf(stderr,"*info* %lf %lf %lf %lf\n", median, three9, four9, five9);

  histFree(&h);
  return 0;
}
