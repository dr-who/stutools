#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <unistd.h>
#include <assert.h>

#include "utils.h"
#include "histogram.h"
#include "numList.h"

void histSetup(histogramType *h, const double min, const double max, const double binscale)
{
  h->min = min; // 0 s
  h->max = max; // 10 s
  h->binScale = ceil(1 / binscale); // 0.01ms for 1 seconds is 100,000 bins
  h->arraySize = h->max * h->binScale;
  CALLOC(h->bin, h->arraySize + 1, sizeof(size_t));
  CALLOC(h->binSum, h->arraySize + 1, sizeof(size_t));
  h->dataSum = 0;
  h->dataCount = 0;
  h->dataSummed = 0;
  nlInit(&h->nl, 1000000); // keep 1 million positions
  //  fprintf(stderr,"*info* [%lf,%lf], binscale %zd, numBins %zd\n", h->min, h->max, h->binScale, h->arraySize);
}

void histAdd(histogramType *h, double value)
{
  size_t bin = 0;
  nlAdd(&h->nl, value);

  if (value < 0) value = 0;

  bin = (size_t) (value * h->binScale);
  if (bin > h->arraySize) {
    bin = h->arraySize;
  }
  h->bin[bin]++;
  h->dataSum += value;
  h->dataCount++;
  //  fprintf(stderr,"value %lf, sum %lf  count %zd, mean %lf\n", value * 1000, h->dataSum, h->dataCount, 1000.0*histMean(h));

}

size_t histCount(histogramType *h)
{
  return h->dataCount;
}

size_t histMaxCount(histogramType *h)
{
  size_t max = 0;
  for (size_t i = 0; i <= h->arraySize; i++) {
    if (h->bin[i] > max) {
      max = h->bin[i];
    }
  }
  return max;
}

// mean is 0 if no data
double histMean(const histogramType *h)
{
  return h->dataSum * 1.0 / h->dataCount;
}


void histSum(histogramType *h)
{
  double sum = 0;
  for (size_t i = 0; i <= h->arraySize; i++) {
    sum += h->bin[i];
    h->binSum[i] = sum;
  }
}

double histLowestPresentValue(histogramType *h)
{
  for (size_t i = 0; i <= h->arraySize; i++) {
    if (h->bin[i] > 0) {
      const double value = i * 1.0 / h->binScale;
      return value;
    }
  }
  return 0;
}

double histHighestPresentValue(histogramType *h)
{
  for (int i = h->arraySize; i >= 0; i--) {
    if (h->bin[i] > 0) {
      const double value = i * 1.0 / h->binScale;
      return value;
    }
  }
  return 0;
}

void histSumPercentages(histogramType *h, double *median, double *three9, double *four9, double *five9, const size_t scale)
{
  histSum(h);
  //  assert(h->dataSum);

  size_t maxsum = h->binSum[h->arraySize];
  *median = 0;
  *three9 = 0;
  *four9 = 0;
  *five9 = 0;

  int okmedian = 0, okthree9 = 0, okfour9 = 0, okfive9 = 0;

  for (size_t i = 0; i <= h->arraySize; i++) {
    double value = i * 1.0 / h->binScale;
    if (h->binSum[i] >= maxsum * 0.5 && !okmedian) {
      okmedian = 1;
      *median = value * scale;
    }
    if (h->binSum[i] >= floor(maxsum * 0.999) && !okthree9) {
      okthree9 = 1;
      *three9 = value * scale;
    }

    if (h->binSum[i] >= floor(maxsum * 0.9999) && !okfour9) {
      okfour9 = 1;
      *four9 = value * scale;
    }

    if (h->binSum[i] >= floor(maxsum * 0.99999) && !okfive9) {
      okfive9 = 1;
      *five9 = value * scale;
    }
  }
}





void histSave(histogramType *h, const char *filename, const size_t scale)
{
  histSum(h);
  //  assert(h->dataSum);

  size_t maxvalue = h->binSum[h->arraySize];


  FILE *fp = fopen(filename, "wt");
  if (!fp) {
    perror(filename);
    return;
  }

  for (size_t i = 0; i <= h->arraySize; i++) {
    fprintf(fp, "%.3lf\t%zd\t%.1lf\n", i * scale * 1.0 / h->binScale, h->bin[i], 100.0 * h->binSum[i] / maxvalue);
    if (h->binSum[i] == maxvalue) {
      break; // stop when we hit the max
    }
  }
  fclose(fp);
  // calculate
}

void histFree(histogramType *h)
{
  if (h->bin) {
    free(h->bin);
  }
  if (h->binSum) {
    free(h->binSum);
    h->binSum = NULL;
  }
  nlFree(&h->nl);
}

