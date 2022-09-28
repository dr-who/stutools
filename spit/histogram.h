#ifndef HISTOGRAM_H
#define HISTOGRAM_H


typedef struct {
  double min;
  double max;
  size_t binScale;
  size_t arraySize;
  size_t *bin;
  size_t *binSum;
  double dataSum;
  size_t dataCount;
  int    dataSummed;
} histogramType;

void histSetup(histogramType *h, const double min, const double max, const double binscale);
double histMean(const histogramType *h);
void histAdd(histogramType *h, double value);
void histFree(histogramType *h);
void histSave(histogramType *h, const char *filename, const size_t scale);
void histSum(histogramType *h);
void histSumPercentages(histogramType *h, double *median, double *three9, double *four9, double *five9, const size_t scale);
size_t histCount(histogramType *h);
size_t histMaxCount(histogramType *h);
double histLowestPresentValue(histogramType *h);
double histHighestPresentValue(histogramType *h);

#endif
