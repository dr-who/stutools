#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <malloc.h>
#include <math.h>
#include <limits.h>
#include <assert.h>
#include <string.h>

#include "numList.h"


void nlInit(numListType *n, int window) {
  if (window < 0) window = 1;
  n->num = 0;
  n->values = calloc(window, sizeof(pointType));
  assert(n->values);
  n->sortedValue = 0;
  n->sortedAge = 1;
  n->ever = 0;
  n->addat = 0;
  n->window = window;
  n->label = NULL;
}

void nlFree(numListType *n) {
  if (n->values) free(n->values);
  if (n->label) free(n->label);
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

static int nl_sortfunctionvalue(const void *origp1, const void *origp2)
{
  pointType *p1 = (pointType*)origp1;
  pointType *p2 = (pointType*)origp2;
  
  if (p1->value < p2->value) return -1;
  else if (p1->value > p2->value) return 1;
  else return 0;
}


static int nl_sortfunctionage(const void *origp1, const void *origp2)
{
  pointType *p1 = (pointType*)origp1;
  pointType *p2 = (pointType*)origp2;
  
  if (p1->age < p2->age) return -1;
  else if (p1->age > p2->age) return 1;
  else return 0;
}



size_t nlAdd(numListType *n, double value) {

  if (n->sortedAge == 0) {
    //    fprintf(stderr,"sort by age\n");
    if (n->num >= 2) {
      qsort(n->values, n->num, sizeof(pointType), nl_sortfunctionage);
    }
    n->sortedAge = 1;
    n->sortedValue = 0;
    n->addat = n->num; // add at the end
  }

  if (n->addat >= n->window) {
    n->addat = n->addat - n->window;
  }

  n->values[n->addat].value = value;
  n->values[n->addat].age = ++(n->ever);
  n->num++; if (n->num > n->window) n->num = n->window;
  n->addat++;

  //  nlDump(n);

  return n->num;
}


/*double nlSum(numListType *n) {
  return n->sum;
  }*/

void nlSort(numListType *n) {
  if (!n->sortedValue) {
    //    fprintf(stderr,"sort by value\n");
    qsort(n->values, n->num, sizeof(pointType), nl_sortfunctionvalue);
    //    nlDump(n);
    n->sortedValue = 1;
    n->sortedAge = 0;
  }
}

double nlSortedPos(numListType *n, double pos) {
  nlSort(n);
  assert(pos>=0);
  assert(pos <= 1);

  size_t i = ceil((n->num - 1) * pos);
  //  fprintf(stderr,"** sorted pos %lf. Index pos 0..%zd, index = %zd\n", pos, n->num-1, i);
  //  nlDump(n);


  //    fprintf(stdout,"*info* index %zd (%zd and %lf)\n", i, n->num, pos);
    /*  if ((i % 2) == 1) {
    return (n->values[i].value + n->values[i+1].value) / 2.0;
  } else {
  // odd*/

  if (i >= n->num) {
    return NAN;
  } else {
    return n->values[i].value;
  }
}


double nlSortedSmoothed(numListType *n) {
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


double nlMedian(numListType *n) {
  if (n->num > 0) {
    return nlSortedPos(n, 0.5);
  } else {
    return NAN;
  }
}

double nlMean(numListType *n) {
  double sum = 0;
  
  for (size_t i = 0; i < n->num; i++) {
    sum += n->values[i].value;
  }
  if (n->num > 0) {
    sum = sum / n->num;
  }
  return sum;
}


double nlSD(numListType *l) {
  double mean = nlMean(l);
  double sum = 0;

  if (l->num >= 2) {
    for (size_t i = 0; i < l->num; i++) {
      double val = l->values[i].value - mean;
      sum += (val * val);
    }
    return sqrt(sum / (l->num-1));
  } else {
    return NAN;
  }
}

double nlSEM(numListType *l) {
  return nlSD(l) / sqrt(nlN(l));
}
    

void nlDump(numListType *n) {
  for (size_t i = 0; i < n->num; i++) {
    fprintf(stdout,"%g (age %zd)\n", n->values[i].value, n->values[i].age);
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
    *r = ((n * sumxy) - (sumx * sumy)) / sqrt((n * sumx2 - sumx*sumx) * (n * sumy2 - sumy*sumy));
  } else {
    fprintf(stderr,"*warning* the N values are different\n");
    *r = NAN;
  }
}

void nlUnbiasedSD(numListType *n1, numListType *n2, const double r, double *unsd) {

  numListType diff;
  nlInit(&diff, 10000);
  
  *unsd  = 0;
  if (nlN(n1) == nlN(n2)) {
    size_t n = nlN(n1);
    for (size_t i = 0; i < n; i++) {
      double x = n1->values[i].value;
      double y = n2->values[i].value;
      nlAdd(&diff, x - y);
    }
    fprintf(stderr,"mean of diff = %.4lf\n  SD of diff = %.4lf\n SEM of diff = %.4lf\n", nlMean(&diff)+r*0, nlSD(&diff), nlSEM(&diff));

    double t = fabs(nlMean(&diff ) / nlSEM(&diff));
    fprintf(stderr,"t = %.4lf\n", t);
    size_t df = nlN(&diff) - 1;
    
    fprintf(stderr,"df = %zd\n", df);

    double p = loadTTable(df, 2, 0.05);
    fprintf(stderr,"critical value p = %.4lf\n",p);
    if (t > p) {
      fprintf(stderr,"*info* as %.4lf > %.4lf confident of difference at 0.05 level\n", t, p);
    } else {
      fprintf(stderr,"*info* as %.4lf not > %.4lf, then not confident of difference at 0.05 level\n", t, p);
    }
    
    nlFree(&diff);
  } else {
    fprintf(stderr,"*warning* the N values are different\n");
    *unsd= NAN;
  }
}
    
  
double loadTTable(size_t df, size_t tail, double a) {
  double ret = NAN;
  
  a = 0.05; // hard coded
  fprintf(stderr,"*info* load table %zd, %zd, %.4lf\n", df, tail, a);
  FILE *fp = fopen("crit-t.dat", "rt");
  char s[1000];
  char *last = NULL;
  if (df > 200) df = 200;
  if (fp) {
    while (fgets(s, 1000, fp) != NULL) {
      char *first = strtok_r(s, "\t \n", &last);
      if (first && (atoi(first) == (int)df)) {
	char *second = strtok_r(last, "\t \n", &last);
	char *third = strtok_r(last, "\t \n", &last);

	if (tail == 1) {
	  ret = atof(second);
	} else { // tail == 2
	  ret = atof(third);
	}
      }
    } 
    fclose(fp);
  } else {
    fprintf(stderr,"*error* the critical t table couldn't be loaded\n");
  }
  return ret;
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
