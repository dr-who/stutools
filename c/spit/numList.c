#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <math.h>
#include <limits.h>
#include <assert.h>
#include <string.h>

#include "linux-headers.h"

#include "utils.h"
#include "numList.h"

void nlShrink(numListType *n, size_t nn) {
    pthread_mutex_lock(&n->lock);
    n->num = MIN(nn, n->num);
    n->addat = n->num;
    pthread_mutex_unlock(&n->lock);
}

void nlClear(numListType *n) {
    nlShrink(n, 0);
}

void nlInit(numListType *n, int window) {
    memset(n, 0, sizeof(numListType));
    if (window <= 0) window = 1;
    //  n->num = 0;
    assert(window);
    n->values = calloc(window, sizeof(pointType));
    assert(n->values);
    n->ever = 0;
    //  n->addat = 0;
    n->window = window;
    n->label = NULL;
    nlClear(n);
    if (pthread_mutex_init(&n->lock, NULL) != 0) {
        fprintf(stderr, "*error* mutex lock has failed\n");
        exit(1);
    }
}


void nlFree(numListType *n) {
  if (n->values) free(n->values);
  if (n->label) free(n->label);
  pthread_mutex_destroy(&n->lock);
  memset(n, 0, sizeof(numListType));
}


size_t nlN(numListType *n) {
    return n->num;
}


void nlSetLabel(numListType *n, const char *label) {
    if (label == NULL) {
        n->label = strdup("");
    } else {
        n->label = strdup(label);
    }
}

char *nlLabel(numListType *n) {
    return n->label;
}

static int sortdouble(const void *origp1, const void *origp2) {
    const double *d1 = (double *) origp1;
    const double *d2 = (double *) origp2;
    if (*d1 < *d2) return -1;
    else if (*d1 > *d2) return 1;
    else return 0;
}


size_t nlAdd(numListType *n, const double value) {
    pthread_mutex_lock(&n->lock);
    if (n->addat >= n->window) {
        n->addat = n->addat - n->window;
    }
    assert(n->addat < n->window);

    n->values[n->addat].value = value;
    n->values[n->addat].age = ++(n->ever);
    n->num++;
    if (n->num > n->window) n->num = n->window;
    n->addat++;

    pthread_mutex_unlock(&n->lock);
    return n->num;
}


// 0 first (min)
// 0.5 is median
// -1 is last (max)
// >1 is position
double nlSortedPos(numListType *n, const double pos) {
    double ret = NAN;

    if ((pos != 0) && (pos < 1)) {
        if (pos >= 0.999 && n->num < 10000) return NAN;
        if (pos >= 0.9999 && n->num < 100000) return NAN;
        if (pos >= 0.99999 && n->num < 1000000) return NAN;
    }

    pthread_mutex_lock(&n->lock);
    size_t nnum = n->num;
    double *d = calloc((nnum + 1), sizeof(double)); // add one so 0 values have a mean of 0
    assert(d);

    for (size_t i = 0; i < nnum; i++) {
        d[i] = n->values[i].value;
    }
    qsort(d, nnum, sizeof(double), sortdouble);
    /*  for (size_t i = 0; i < nnum; i++) {
        printf("%lf ", d[i]);
      }
      printf("\n");*/

    pthread_mutex_unlock(&n->lock);


    if (nnum == 0) {
        ret = NAN;
    } else if (pos == 0) {
        ret = d[0];
    } else if (pos == -1) {
        ret = d[nnum - 1];
    } else if ((nnum > 0) && (pos < 1)) {
        size_t mid = (size_t) ((nnum - 1) * pos);
        if ((nnum % 2) == 0) {
            // even
            ret = (d[mid] + d[mid + 1]) / 2.0;
        } else {
            ret = d[mid];
        }
    } else if (pos < nnum) {
        ret = d[(size_t) pos];
    } else {
        fprintf(stderr, "*error* pos get out of range [0..%zd) ask %lf\n", nnum, pos);
    }
    free(d);

    return ret;
}

double nlMin(numListType *n) {
    return nlSortedPos(n, 0);
}


double nlMax(numListType *n) {
    return nlSortedPos(n, -1);
}

double nlMedian(numListType *n) {
    return nlSortedPos(n, 0.5);
}


/*double nlSortedSmoothed(numListType *n) {
  nlSort(n);
  double v = 0;
  double weight = 0;
  double tot = 0;
  size_t oldest = INT_MAX;

  for (size_t i = 0; i < n->num; i++) {
    if (n->values[i].age < oldest) oldest = n->values[i].age;
  }
  
  for (size_t i = 0; i < n->num; i++) {
    v = n->values[i].age - oldest + 1;
    assert(v > 0);

    weight += v;
    tot += n->values[i].value * v;
  }
  if (weight > 0) {
    tot = tot / weight;
  }

  return tot;
}

*/


double nlMode(numListType *n, size_t buckets, size_t div) {
    size_t bu[buckets];
    for (size_t i = 0; i < buckets; i++) {
        bu[i] = 0;
    }

    for (size_t i = 0; i < n->num; i++) {
        size_t v = fmod(n->values[i].value, buckets);
        v = v / div;
        v = v * div;
        if (v < buckets) {
            bu[v]++;
            //      fprintf(stdout,"adding %zd, count = %zd\n", v, bu[v]);
        }
    }

    int max = -1, maxpos = -1;
    for (size_t i = 0; i < buckets; i++) {
        if ((int) bu[i] > max) {
            max = bu[i];
            maxpos = i;
        }
    }
    //  fprintf(stdout,"mode value is %zd, count %zd\n", maxpos, bu[maxpos]);
    if (max <= 1) {
        return NAN;
    } else { // if more than 3
        return (double) maxpos;
    }
}


double nlMean(numListType *n) {
    pthread_mutex_lock(&n->lock);
    double sum = 0;
    const size_t nnum = n->num;

    for (size_t i = 0; i < nnum; i++) {
        sum += n->values[i].value;
    }
    if (nnum > 0) {
        sum = sum / nnum;
    } else {
        sum = NAN;
    }
    pthread_mutex_unlock(&n->lock);
    return sum;
}


double nlSD(numListType *l) {
    const double mean = nlMean(l);

    pthread_mutex_lock(&l->lock);
    double sum = 0, ret = 0;
    const size_t nnum = l->num;

    if (nnum >= 2) {
        for (size_t i = 0; i < nnum; i++) {
            double val = l->values[i].value - mean;
            sum += (val * val);
        }
        ret = sqrt(sum / (nnum - 1));
    } else {
        ret = NAN;
    }
    pthread_mutex_unlock(&l->lock);
    return ret;
}

double nlSEM(numListType *l) {
    return nlSD(l) / sqrt(nlN(l));
}


void nlDump(numListType *n) {
    for (size_t i = 0; i < n->num; i++) {
        fprintf(stdout, "%g (age %zd)\n", n->values[i].value, n->values[i].age);
    }
}


void nlCorrelation(numListType *n1, numListType *n2, double *r) {
    *r = 0;
    if (nlN(n1) == nlN(n2)) {
        double sumxy = 0;
        double sumx = 0;
        double sumx2 = 0;
        double sumy = 0;
        double sumy2 = 0;

        size_t n = nlN(n1);
        for (size_t i = 0; i < n; i++) {
            const double x = n1->values[i].value;
            const double y = n2->values[i].value;
            sumxy += (x * y);
            sumx += (x);
            sumx2 += (x * x);
            sumy += (y);
            sumy2 += (y * y);
        }
        *r = ((n * sumxy) - (sumx * sumy)) / sqrt((n * sumx2 - sumx * sumx) * (n * sumy2 - sumy * sumy));
    } else {
        fprintf(stderr, "*warning* the N values are different\n");
        *r = NAN;
    }
}

void nlUnbiasedSD(numListType *n1, numListType *n2, const double r, double *unsd) {

    numListType diff;
    nlInit(&diff, 1000000);

    *unsd = 0;
    if (nlN(n1) == nlN(n2)) {
        size_t n = nlN(n1);
        for (size_t i = 0; i < n; i++) {
            double x = n1->values[i].value;
            double y = n2->values[i].value;
            nlAdd(&diff, x - y);
        }
        fprintf(stdout, "mean of diff = %.4lf\n  SD of diff = %.4lf\n SEM of diff = %.4lf\n", nlMean(&diff) + r * 0,
                nlSD(&diff), nlSEM(&diff));

        double t = fabs(nlMean(&diff) / nlSEM(&diff));
        fprintf(stdout, "t = %.4lf\n", t);
        size_t df = nlN(&diff) - 1;

        fprintf(stdout, "df = %zd\n", df);

        double p = loadTTable(df, 2, 0.05);
        fprintf(stdout, "critical value p = %.4lf\n", p);
        if (t > p) {
            fprintf(stdout, "*info* as %.4lf > %.4lf confident of difference at 0.05 level\n", t, p);
        } else {
            fprintf(stdout, "*info* as %.4lf not > %.4lf, then not confident of difference at 0.05 level\n", t, p);
        }

        nlFree(&diff);
    } else {
        fprintf(stderr, "*warning* the N values are different\n");
        *unsd = NAN;
    }
}

#include "crit-t.h"

double loadTTable(size_t df, size_t tail, double a) {
  if (tail < 1) tail = 1;
    a = 0.05; // hard coded
    fprintf(stderr, "*info* t-distribution table, df=%zd, tail=%zd, confidence=%.4lf\n", df, tail, a);
    if (df > 199) df = 199;

    return tvaluesp[df-1][tail];
}


/*
int main() {
  numListType n;
  nlInit(&n, 1000000);
  double j;
  for (size_t i = 0 ; i < 100000000; i++) {
    nlAdd(&n, i);
  }
  j = nlMedian(&n);
  fprintf(stderr,"median %lf\n", j);
  return 0;
}
*/



void nlSummary(numListType *nl) {
    fprintf(stdout,
            "mean: \t%lf\nmin: \t%lf\nQ25%%: \t%lf\nMedian:\t%lf\nQ75%%: \t%lf\nmax: \t%lf\nN:   \t%zd\nSD: \t%lf\nSEM: \t%lf\n",
            nlMean(nl), nlMin(nl), nlSortedPos(nl, 0.25), nlMedian(nl), nlSortedPos(nl, 0.75), nlMax(nl), nlN(nl),
            nlSD(nl), nlSEM(nl));
    //fprintf(stdout,"99.9%%:  \t%lf\n", nlSortedPos(nl, 0.999));
    //  fprintf(stdout,"99.99%%: \t%lf\n", nlSortedPos(nl, 0.9999));
    //  fprintf(stdout,"99.999%%:\t%lf\n", nlSortedPos(nl, 0.99999));
}

void nlBriefSummary(numListType *nl) {
    fprintf(stdout, "mean: \t%lf\nmin: \t%lf\nmax: \t%lf\n",
            nlMean(nl), nlMin(nl), nlMax(nl));
}

