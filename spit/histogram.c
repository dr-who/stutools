#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <unistd.h>
#include <assert.h>

#include "utils.h"
#include "histogram.h"
#include "numList.h"

void histSetup(histogramType *h, const double min, const double max, const double binsize) {
    h->min = min; // 0 s
    h->max = max; // 10 s
    if (binsize == 0) {
      h->binSize = 0.1;
    } else {
      //     h->binSize = pow(10, ceil(log10(binsize))); // 0.01ms for 1 seconds is 100,000 bins
      h->binSize = binsize;
    }
    assert(h->binSize > 0);
    assert(h->max >= h->min);
    h->arraySize = ceil( (h->max - h->min) / h->binSize );
    CALLOC(h->bin, h->arraySize + 1, sizeof(size_t));
    CALLOC(h->binSum, h->arraySize + 1, sizeof(size_t));
    h->maxSeen = min;
    h->minSeen = max;
    h->showASCII = 0;
    h->dataSum = 0;
    h->dataCount = 0;
    h->dataSummed = 0;
    nlInit(&h->nl, 1000000); // keep 1 million positions
    //  fprintf(stderr,"*info* [%lf,%lf], binSize %zd, numBins %zd\n", h->min, h->max, h->binSize, h->arraySize);
}


double getIndexToXValueScale(histogramType *h, const size_t index, const size_t maxindex, const size_t w) {
    if (h) {}
    double rat = index * 1.0 / maxindex;
    return (w - 1) * rat;
}

double getIndexToYValueScale(histogramType *h, const size_t index, const size_t y) {
    size_t mv = 0;
    for (size_t i = 0; i <= h->arraySize; i++) {
        if (h->bin[i] > mv) mv = h->bin[i];
    }
    double rat = h->bin[index] * 1.0 * (y - 1) / mv;
    return rat;
}

double getIndexToXValue(histogramType *h, const size_t index) {
    assert(index <= h->arraySize);
    double val = h->min + (index * 1.0 * h->binSize);
    //    fprintf(stderr,"*val is %lf*, range is %lf   %lf, index %zd, binSize %lf\n", val, h->min, h->max, index, h->binSize);
    assert(val >= h->min);
    assert(val <= h->max);
    return val;
}

double getIndexToYValue(histogramType *h, const size_t index) {
    assert(index <= h->arraySize);
    return h->bin[index];
}

size_t getXIndexFromValue(histogramType *h, const double val) {
    if (val < h->min || val > h->max) {
      //        fprintf(stderr, "** value %lf is outside the range [%lf, %lf]\n", val, h->min, h->max);
        return 0;
    }
    assert(val >= h->min);
    assert(val <= h->max);
    size_t ind = (val - h->min) / h->binSize;
    assert(ind <= h->arraySize);
    return ind;
}


double *histScanInternal(const char *fn, double *min, double *max, double *binsize, size_t *n) {
    FILE *stream;
    char *line = NULL;
    size_t len = 0;
    ssize_t nread;
    double *values = NULL;

    stream = fopen(fn, "rt");
    if (!stream) {
        perror("fopen");
        exit(EXIT_FAILURE);
    }

    double last = -9e9, gapmax = 9e9, gap = 0;
    size_t count = 0;
    int first = 1;

    while ((nread = getline(&line, &len, stream)) != -1) {
        double val = 0;
        long freq;
        int p = sscanf(line, "%lf %ld", &val, &freq);
        if (p == 1) {
            freq = 1;
            p = 2;
        }

        if ((p == 2) && (freq > 0)) {
            for (long q = 0; q < freq; q++) {
                count++;
                values = (double *) realloc(values, count * sizeof(double));
                values[count - 1] = val;
            }

            if (first || (val < *min)) {
                *min = val;
                first = 0;
            }
            if (first || (val > *max)) {
                *max = val;
                first = 0;
            }

            if (last != -9e9) {
                gap = val - last;
                if (gap > 0 && gap < gapmax) {
                    gapmax = gap;
                    *binsize = gapmax;
                }
            }
            last = val;
        }
    }


    free(line);
    fclose(stream);
    *n = count;
    return values;
}


void histLoad(histogramType *h, const char *fn) {
    double min, max, bin;
    size_t n;

    // read to find min, max and binSize
    double *values = histScanInternal(fn, &min, &max, &bin, &n);

    histSetup(h, min, max, bin);
    for (size_t i = 0; i < n; i++) {
        histAdd(h, values[i]); // add in the error
    }
    free(values);

    histSum(h); // calc ranges

    fprintf(stderr, "*info* '%s': sum counts %zd, min %lf, max %lf, binsize %lf, elements[] %zd\n", fn, n, min, max, bin, h->arraySize);
    
}

void histAdd(histogramType *h, double value) {
    size_t bin = 0;
    if ((h->dataCount == 0) || (value > h->maxSeen)) {
        h->maxSeen = value;
    }
    if ((h->dataCount == 0) || (value < h->minSeen)) {
        h->minSeen = value;
    }

    nlAdd(&h->nl, value);

    bin = getXIndexFromValue(h, value);
    h->bin[bin]++;
    h->dataSum += value;
    h->dataCount++;
    //  fprintf(stderr,"value %lf, sum %lf  count %zd, mean %lf\n", value * 1000, h->dataSum, h->dataCount, 1000.0*histMean(h));

}

size_t histCount(histogramType *h) {
    return h->dataCount;
}

size_t histMaxCount(histogramType *h) {
    size_t max = 0;
    for (size_t i = 0; i <= h->arraySize; i++) {
        if (h->bin[i] > max) {
            max = h->bin[i];
        }
    }
    return max;
}

// mean is 0 if no data
double histMean(const histogramType *h) {
    return h->dataSum * 1.0 / h->dataCount;
}


void histSum(histogramType *h) {
    double sum = 0;
    for (size_t i = 0; i <= h->arraySize; i++) {
        sum += h->bin[i];
        h->binSum[i] = sum;
    }
}

double histLowestPresentValue(histogramType *h) {
    for (size_t i = 0; i <= h->arraySize; i++) {
      if (h->bin[i] > 0) {
	const double value = getIndexToXValue(h, i);
	return value;
      }
    }
    return NAN;
}

double histHighestPresentValue(histogramType *h) {
    for (int i = h->arraySize; i >= 0; i--) {
        if (h->bin[i] > 0) {
            const double value = getIndexToXValue(h, i);
            return value;
        }
    }
    return NAN;
}

double histConsistency(histogramType *h) {
    if (h->arraySize < 1) {
        return 0;
    }

    size_t valueCount = h->bin[0];

    for (size_t i = 0; i < h->arraySize; i++) {
        if (h->bin[i] > valueCount) {
            valueCount = h->bin[i];
        }
    }

    double consistency = NAN;
    for (size_t i = 0; i < h->arraySize; i++) {
      if (h->bin[i] == valueCount) {
	// max
	
	const double value = getIndexToXValue(h, i);
	consistency = 100.0 * (valueCount * 1.0 / h->dataCount);
    
	fprintf(stderr, "*info* range [%g-%g) (n=%zd), representing %.1lf%%\n", value,
		value + 1.0 * h->binSize, valueCount, consistency);
      }
    }
    return consistency;
}

void
histSumPercentages(histogramType *h, double *median, double *three9, double *four9, double *five9, const size_t scale) {
    *median = nlSortedPos(&h->nl, 0.5) * scale;
    *three9 = nlSortedPos(&h->nl, 0.999) * scale;
    *four9 = nlSortedPos(&h->nl, 0.9999) * scale;
    *five9 = nlSortedPos(&h->nl, 0.99999) * scale;
}


void histSave(histogramType *h, const char *filename) {
    histSum(h);
    //  assert(h->dataSum);

    size_t maxvalue = h->binSum[h->arraySize];


    FILE *fp = fopen(filename, "wt");
    if (!fp) {
        perror(filename);
        return;
    }

    for (size_t i = 0; i <= h->arraySize; i++) {
        fprintf(fp, "%.3lf\t%zd\t%.2lf\n", getIndexToXValue(h, i), h->bin[i], 100.0 * h->binSum[i] / maxvalue);
        if (h->binSum[i] == maxvalue) {
            break; // stop when we hit the max
        }
    }
    fclose(fp);
    // calculate
}

void histFree(histogramType *h) {
    if (h->bin) {
        free(h->bin);
    }
    if (h->binSum) {
        free(h->binSum);
        h->binSum = NULL;
    }
    nlFree(&h->nl);
}

void histWriteGnuplot(histogramType *hist, const char *datafile, const char *gnufile, const char *outpngfile,
                      const char *xlabel, const char *ylabel) {
    double median = 0, three9 = 0, four9 = 0, five9 = 0;
    if (histCount(hist)) {
        histSumPercentages(hist, &median, &three9, &four9, &five9, 1);
        if (isnan(three9)) three9 = 0;
        if (isnan(four9)) four9 = 0;
        if (isnan(five9)) five9 = 0;
    }
    FILE *fp = fopen(gnufile, "wt");
    if (fp) {
        fprintf(fp, "set term png size '1000,800' font 'arial,10'\n");
        fprintf(fp, "set output '%s'\n", outpngfile);
        fprintf(fp, "set key above\n");
        fprintf(fp, "set title 'Response Time Histogram - Confidence Level Plot (n=%zd)'\n", hist->dataCount);
        fprintf(fp, "set nonlinear x via log10(x)/log10(2) inverse 2**x\n"); // xtics like 1, 2, 4, 8 ms
        fprintf(fp, "set xtics 2 logscale out\n");
        fprintf(fp, "set log x\n");
        fprintf(fp, "set log y\n");
        fprintf(fp, "set yrange [0.9:%lf]\n", histMaxCount(hist) * 1.1);
        fprintf(fp, "set y2tics 10 out\n");
        fprintf(fp, "set grid\n");
        fprintf(fp, "set xrange [%lf:%lf]\n", histLowestPresentValue(hist) * 0.66, histHighestPresentValue(hist) * 1);
        fprintf(fp, "set y2range [0:100]\n");
        fprintf(fp, "set xlabel '%s'\n", xlabel);
        fprintf(fp, "set ylabel '%s'\n", ylabel);
        fprintf(fp, "set y2label 'Confidence level'\n");
        fprintf(fp,
                "plot '%s' using ($1+%lf):2 with boxes title 'Latency', '%s' using 1:3 with lines title '%% Confidence' axes x1y2,'<echo %lf 100000' with imp title 'ART=%.3lf' axes x1y2, '<echo %lf 100000' with imp title '99.9%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.99%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.999%%=%.2lf' axes x1y2\n",
                datafile, (1.0 * hist->binSize) / 2.0, datafile, median, median, three9, three9, four9, four9, five9,
                five9);
    } else {
        perror("filename");
    }
    if (fp) {
        fclose(fp);
        fp = NULL;
    }
}

double histSample(histogramType *h) {
    double value = 0;

    const size_t n1 = lrand48();
    size_t n = n1 % (h->binSum[h->arraySize] + 1);

    for (size_t i = 0; i <= h->arraySize; i++) {
        if (n <= h->binSum[i]) {
            value = getIndexToXValue(h, i);
            break;
        }
    }
    if (value < h->min) value = h->min;
    if (value > h->max) value = h->max;

    return value;
}


double binarySample(histogramType *h) {
    double value = 0;

    for (size_t i = 0; i <= h->arraySize; i++) {
      if (h->bin[i] > 0) {
	const double dr = drand48();
	const double v = getIndexToXValue(h, i);
	if (dr < (1.0 * h->bin[i] / 100.0)) {
	  //	  fprintf(stdout,"yes for %lf\n", v);
	  value += v;
	} else {
	  //	  fprintf(stdout,"no for %lf\n", v);
	}
      }
    }
    
    return value;
}
  


void asciiField(histogramType *h, const size_t x, const size_t y, const char *title) {

    char *field = calloc(x * y, sizeof(char));
    assert(field);
    memset(field, ' ', x * y);

    //  printf("index %zd, maxseen = %lf\n", getXIndexFromValue(h, h->maxSeen), h->maxSeen);
    size_t maxi = getXIndexFromValue(h, h->maxSeen);
    int drawn = 0;
    for (size_t i = 0; i <= maxi; i++) {
        if (h->bin[i]) {

            // i index from min to max;
            double xpos = getIndexToXValueScale(h, i, maxi, (double) x);
            double ypos = ceil(getIndexToYValueScale(h, i, (double) y));

            //      fprintf(stderr,"%lf %lf\n", xpos, ypos);

            for (size_t j = 0; j <= ypos; j++) {
                size_t p = j * x + xpos;
                assert(p < x * y);
                field[p] = '*';
                drawn = 1;
            }
        }
    }
    if (drawn) {
        printf("\n"); // blank line before
        printf("+ %30s %s\n", "", title);
        for (size_t j = y - 1; j > 0; j--) {
            printf("|");
            for (size_t i = 0; i < x; i++) {
                size_t p = j * x + i;
                assert(p < x * y);

                printf("%c", field[p]);
            }
            printf("\n");
        }
        printf("+");
        for (size_t i = 0; i < x; i++) {
            printf("-");
        }
        printf("\n");
        printf("%-10.2lf%-25s%10.2lf%25s%10.2lf\n", h->minSeen, "", (h->maxSeen + h->minSeen) / 2, "", h->maxSeen);
    }

}

