#ifndef __NUMLIST_H
#define __NUMLIST_H

#include <math.h>
#include "pthread.h"

typedef struct {
    double value;
    size_t age;
} pointType;

typedef struct {
    size_t num;
    size_t addat;
    pointType *values;
    size_t ever, window;
    //  double sum;
    char *label;
    pthread_mutex_t lock;
} numListType;


void nlInit(numListType *n, int window);

void nlClear(numListType *n);

void nlShrink(numListType *n, size_t nn);

void nlFree(numListType *n);

size_t nlAdd(numListType *n, double value);

double nlSum(numListType *n);

void nlSort(numListType *n);

void nlSetLabel(numListType *n, const char *label);

char *nlLabel(numListType *n);

double nlSortedPos(numListType *n, const double pos);

double nlSortedSmoothed(numListType *n);

double nlMedian(numListType *n);

double nlMean(numListType *n);

size_t nlN(numListType *n);

double nlSD(numListType *l);

double nlSEM(numListType *l);

double nlMin(numListType *l);

double nlMax(numListType *l);

void nlCorrelation(numListType *n1, numListType *n2, double *r);

void nlUnbiasedSD(numListType *n1, numListType *n2, const double r, double *unsd);

void nlDump(numListType *n);

double nlMode(numListType *n, size_t buckets, size_t div);


double loadTTable(size_t df, size_t tail, double a);

void nlSummary(numListType *nl);


#endif

