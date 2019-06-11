#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <unistd.h>
#include <assert.h>

#include "utils.h"
#include "histogram.h"

void histSetup(histogramType *h) {
  h->min = 0; // 0 s
  h->max = 10; // 10 s
  h->binScale = ceil(1 / 0.0001); // 0.1ms for 1 seconds is 10,000 bins
  h->arraySize = h->max * h->binScale;
  CALLOC(h->bin, h->arraySize + 1, sizeof(size_t));
  CALLOC(h->binSum, h->arraySize + 1, sizeof(size_t));
  h->dataSum = 0;
  h->dataCount = 0;
  h->dataSummed = 0;
  //  fprintf(stderr,"*info* [%lf,%lf], binscale %zd, numBins %zd\n", h->min, h->max, h->binScale, h->arraySize);
}

void histAdd(histogramType *h, double value) {
  size_t bin = 0;

  if (value < 0) value = 0;
  
  bin = (size_t) (value * h->binScale);
  if (bin > h->arraySize) {
    bin = h->arraySize;
  }
  h->bin[bin]++;
  h->dataSum += value;
  h->dataCount++;
}

double histMean(const histogramType *h) {
  return h->dataSum / h->dataCount;
}


void histSum(histogramType *h) {
  double sum = 0;
  for (size_t i = 0; i <= h->arraySize; i++) {
    sum += h->bin[i];
    h->binSum[i] = sum;
  }

  h->dataSum = 1;
}

void histSumPercentages(histogramType *h, double *median, double *three9, double *four9, double *five9) {
  histSum(h);
  assert(h->dataSum);

  size_t maxsum = h->binSum[h->binScale];
  *median = 0;
  *three9 = 0;
  *four9 = 0;
  *five9 = 0;

  int okmedian = 0, okthree9 = 0, okfour9 = 0, okfive9 = 0;
  
  for (size_t i = 0; i <= h->arraySize; i++) {
    double value = i * 1.0 / h->binScale;
    if (h->binSum[i] >= maxsum * 0.5 && !okmedian) {
      okmedian = 1;
      *median = value;
    }
    if (h->binSum[i] >= maxsum * 0.999 && !okthree9) {
      okthree9 = 1;
      *three9 = value;
    }
    
    if (h->binSum[i] >= maxsum * 0.9999 && !okfour9) {
      okfour9 = 1;
      *four9 = value;
    }
    
    if (h->binSum[i] >= maxsum * 0.99999 && !okfive9) {
      okfive9 = 1;
      *five9 = value;
    }
  }
}

  
  
    

void histSave(histogramType *h, const char *filename) {
  histSum(h);
  assert(h->dataSum);
  // maxsum
  

  FILE *fp = fopen(filename, "wt");
  if (!fp) {perror(filename); return;}

  for (size_t i = 0; i <= h->arraySize; i++) {
    fprintf(fp, "%.5lf\t%zd\t%zd\n", i * 1.0 / h->binScale, h->bin[i], h->binSum[i]);
  }
  fclose(fp);
  // calculate
}

void histFree(histogramType *h) {
  if (h->bin) {
    free(h->bin);
    free(h->binSum);
    h->bin = NULL;
  }
}
  


  

