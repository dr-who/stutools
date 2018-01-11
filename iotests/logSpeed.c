#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <assert.h>

#include "logSpeed.h"
#include "utils.h"


void logSpeedInit(volatile logSpeedType *l) {
  l->starttime = timedouble();
  l->lasttime = l->starttime;
  l->num = 0;
  l->alloc = 10000;
  l->total = 0;
  l->sorted = 0;
  l->checkpoint = 0;
  l->rawtot = 0;
  CALLOC(l->values, l->alloc, sizeof(double));
  CALLOC(l->rawtime, l->alloc, sizeof(double));
  CALLOC(l->rawvalues, l->alloc, sizeof(double));
  CALLOC(l->rawtotal, l->alloc, sizeof(double));
}

void logSpeedReset(logSpeedType *l) {
  assert(l);
  l->starttime = timedouble();
  l->checkpoint = 0;
  l->lasttime = l->starttime;
  l->num = 0;
  l->total = 0;
  l->sorted = 0;
}


void logSpeedCheckpoint(logSpeedType *l) {
  l->checkpoint = l->total;
}

double logSpeedGetCheckpoint(logSpeedType *l) {
  return l->checkpoint;
}



double logSpeedTime(logSpeedType *l) {
  return l->lasttime - l->starttime;
}



int logSpeedAdd(logSpeedType *l, double value) {
  assert(l);
  const double thistime = timedouble();
  const double timegap = thistime - l->lasttime;
  
  const double v = value / timegap;

  if (isfinite(v)) {
    
    //  fprintf(stderr,"--> adding %.1lf\n", value);
    
    if (l->num >= l->alloc) {
      l->alloc = l->alloc * 2 + 1;
      l->values = realloc(l->values, l->alloc * sizeof(double)); if (!l->values) {fprintf(stderr,"OOM! realloc failed\n");exit(1);}
      l->rawtime = realloc(l->rawtime, l->alloc * sizeof(double));
      l->rawvalues = realloc(l->rawvalues, l->alloc * sizeof(double));
      l->rawtotal = realloc(l->rawtotal, l->alloc * sizeof(double));
      //    fprintf(stderr,"skipping...\n"); sleep(1);
    } 
    // fprintf(stderr,"it's been %zd bytes in %lf time, is %lf, total %zd, %lf,  %lf sec\n", value, timegap, value/timegap, l->total, l->total/logSpeedTime(l), logSpeedTime(l));
    l->values[l->num] = v;
    l->rawtime[l->num] = thistime;
    l->rawtotal[l->num] = l->rawtot;
    l->rawvalues[l->num] = value;
    l->rawtot += value;
    l->sorted = 0;
    l->num++;
  }
  l->total += value;
  l->lasttime = thistime; // update time anyway so we don't lie about the next speed

  return l->num;
}


static int comparisonFunction(const void *p1, const void *p2) {
  double *d1 = (double*)p1;
  double *d2 = (double*)p2;

  if (*d1 < *d2) return -1;
  else if (*d1 > *d2) return 1;
  else return 0;
}

double logSpeedTotal(logSpeedType *l) {
  assert(l);
  return l->total;
}

void logSpeedSort(logSpeedType *l) {
  if (!l->sorted) {
    //    fprintf(stderr,"sorting\n");
    qsort(l->values, l->num, sizeof(double), comparisonFunction);
    l->sorted = 1;
    if (l->num >= 20000) {
      for (size_t i = 0; i < l->num/4; i++) {
	l->values[i] = l->values[i*4];
      }
      l->num = l->num/4;
      //      fprintf(stderr,"1/4\n");
    }
  }
}

double logSpeedMedian(logSpeedType *l) {
  logSpeedSort(l);
  const double med = l->values[l->num / 2];
  return med;
}

double logSpeedMean(logSpeedType *l) {
  if (l->num <= 2) {
    return 0;
  } else {
    return (l->total) / logSpeedTime(l);
  }
}

double logSpeedRank(logSpeedType *l, float rank) {
  if (rank < 0) rank=0;
  if (rank >= 1) rank = 0.9999999;
  logSpeedSort(l);
  return l->values[(size_t)(l->num * rank)];
}

double logSpeedMax(logSpeedType *l) {
  logSpeedSort(l);
  return l->values[l->num - 1];
}

size_t logSpeedN(logSpeedType *l) {
  return l->num;
}

  

void logSpeedFree(logSpeedType *l) {
  free(l->values);
  free(l->rawtime);
  free(l->rawvalues);
  free(l->rawtotal);
  l->values = NULL;
  l->rawtime = NULL;
  l->rawvalues = NULL;
  l->rawtotal = NULL;
}


void logSpeedDump(logSpeedType *l, const char *fn, const int format) {
  //  logSpeedMedian(l);
  //  if (format) {
  //    fprintf(stderr, "*info* logSpeedDump format %d\n", format);
  //  }
  FILE *fp = fopen(fn, "wt");
  if (!format) {// if no format
    for (size_t i = 0; i < l->num; i++) {
      fprintf(fp, "%.6lf\t%.0lf\t%.6lf\t%.0lf\n", l->rawtime[i] - l->starttime, l->rawtotal[i], l->rawtime[i], l->rawvalues[i]);
    }
  } else if (format == JSON) {
    fprintf(fp,"[\n");
    for (size_t i = 0; i < l->num; i++) {
      fprintf(fp, "{\"time\":\"%.6lf\", \"total\":\"%.0lf\"}", l->rawtime[i] - l->starttime, l->rawtotal[i]);
      if (i < l->num-1) fprintf(fp,",");
      fprintf(fp,"\n");
    }
    fprintf(fp,"]\n");
  } else {
    assert(0); // no idea
  }
  fclose(fp);
}


void logSpeedHistogram(logSpeedType *l) {
  const double low = logSpeedRank(l, 0);
  const double high = logSpeedRank(l, 1); // actually 0.9999
  const double N = 10;
  const double gap = (high - low) / N;
  // [0..1] [1..2] .... [9..10]

  fprintf(stderr,"Low: %.1lf MiB/s, High: %.1f MiB/s\n", low / 1024.0/1024, high / 1024.0/1024);

  size_t *hist = calloc(10, sizeof(size_t));

  size_t max = 0;
  for (size_t i = 0; i < logSpeedN(l); i++) {
    //    fprintf(stderr,"...%.1lf\n", l->values[i] / 1024.0/1024);
    size_t index = (l->values[i] - low) / gap;
    if (index > N-1) index = N-1;
    //    fprintf(stderr,"index %zd\n", index);
    hist[index]++;
    if (hist[index] > max) max = hist[index];
  }
  
  for (size_t i = 0; i < N; i++) {
    double newlow = low + (i * gap);
    double newhigh = low + ((i + 1) * gap);
    
    fprintf(stderr,"[%.1lf, %.1lf): ", newlow / 1024.0/1024, newhigh / 1024.0 / 1024);
    double scale = 1.0;
    if (max > 60) {
      scale = 60.0 / max;
    }
    for (size_t j = 0; j < hist[i] * scale; j++) {
      fprintf(stderr,".");
    }
    fprintf(stderr,"\n");
    //%zd\n", i, hist[i]);
  }
  free(hist);
}

