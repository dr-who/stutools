#ifndef HISTOGRAM_H
#define HISTOGRAM_H

#include "numList.h"

typedef struct {
    double min;
    double max;
    double minSeen;
    double maxSeen;
    size_t binScale;
    size_t arraySize;
    size_t *bin;
    size_t *binSum;
    double dataSum;
    size_t dataCount;
    int dataSummed;
    int showASCII;
    numListType nl;
} histogramType;

void histSetup(histogramType *h, const double min, const double max, const double binscale);

double histMean(const histogramType *h);

void histAdd(histogramType *h, double value);

void histFree(histogramType *h);

void histSave(histogramType *h, const char *filename);

void histSum(histogramType *h);

void
histSumPercentages(histogramType *h, double *median, double *three9, double *four9, double *five9, const size_t scale);

size_t histCount(histogramType *h);

size_t histMaxCount(histogramType *h);

double histLowestPresentValue(histogramType *h);

double histHighestPresentValue(histogramType *h);

double histConsistency(histogramType *h);

void histWriteGnuplot(histogramType *hist, const char *datafile, const char *gnufile, const char *outpngfile,
                      const char *xlabel, const char *ylabel);

void histLoad(histogramType *h, const char *fn);

double histSample(histogramType *h);

double getIndexToXValue(histogramType *h, const size_t index);

double getIndexToYValue(histogramType *h, const size_t index);

size_t getXIndexFromValue(histogramType *h, const double val);

double getIndexToXValueScale(histogramType *h, const size_t index, const size_t maxindex, const size_t x);

double getIndexToYValueScale(histogramType *h, const size_t index, const size_t y);

void asciiField(histogramType *h, const size_t x, const size_t y, const char *title);

#endif
