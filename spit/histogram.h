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

void histSetup(histogramType *h);
double histMean(const histogramType *h);
void histAdd(histogramType *h, double value);
void histFree(histogramType *h);
void histSave(histogramType *h, const char *filename);
void histSum(histogramType *h);
void histSumPercentages(histogramType *h, double *median, double *three9, double *four9, double *five9);



#endif
