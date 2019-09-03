#include <stdio.h>
#include <stdlib.h>

#include <malloc.h>
#include <limits.h>
#include <assert.h>

typedef struct {
  size_t num;
  size_t sorted;
  double *values;
  size_t *age;
  size_t ever;
} numListType;

void nlInit(numListType *n) {
  n->num = 0;
  n->values = NULL;
  n->age = NULL;
  n->sorted = 0;
  n->ever = 0;
}

void nlFree(numListType *n) {
  if (n->values) free(n->values);
  if (n->age) free(n->age);
  nlInit(n);
}

   

size_t nlAdd(numListType *n, double value) {

  size_t addat = n->num;

  if (n->num < 100) {
    n->values = realloc(n->values, (n->num+1) * sizeof(double));
    n->age = realloc(n->age, (n->num+1) * sizeof(size_t));
    n->num++;
  } else {
    size_t lowest = (size_t)-1;
    for (size_t i = 0; i < n->num; i++) {
      if (n->age[i] < lowest) {
	lowest = n->age[i];
	addat = i;
      }
    }
    //    fprintf(stderr,"replacing oldest at position %zd\n", addat);
  }

  n->values[addat] = value;
  n->age[addat] = n->ever++;
  n->sorted = 0;

    //
  return n->num;
}


static int nl_sortfunction(const void *origp1, const void *origp2)
{
  double *p1 = (double*)origp1;
  double *p2 = (double*)origp2;
  
  if (*p1 < *p2) return -1;
  else if (*p1 > *p2) return 1;
  else return 0;
}


void nlSort(numListType *n) {
  if (!n->sorted) {
    qsort(n->values, n->num, sizeof(double), nl_sortfunction);
    n->sorted = 1;
  }
}

double nlSortedPos(numListType *n, double pos) {
  nlSort(n);
  assert(pos>=0);
  assert(pos < n->num);
  size_t i = (size_t) ((n->num-1) * pos);

  //  fprintf(stdout,"*info* index %zd (%zd and %lf)\n", i, n->num, pos);
  if ((n->num % 2) == 0) {
    return (n->values[i] + n->values[i+1]) / 2.0;
  } else {
    // odd
    return n->values[i];
  }
}
    

double nlMedian(numListType *n) {
  if (n->num > 0) {
    return nlSortedPos(n, 0.5);
  } else {
    return 0.0/0.0;
  }
}
  

void nlDump(numListType *n) {
  for (size_t i = 0; i < n->num; i++) {
    fprintf(stdout,"%g\n", n->values[i]);
  }
}


int main() {
  numListType n;
  
  nlInit(&n);

  double v = 0;
  while (scanf("%lf", &v) == 1) {
    //    fprintf(stdout,"*info* in %g\n", v);
    nlAdd(&n, v);
    if (n.num > 0) {
      fprintf(stdout,"%g %g %g %g %g\n", nlSortedPos(&n, 0.001), nlSortedPos(&n, 0.01), nlMedian(&n), nlSortedPos(&n, 0.99), nlSortedPos(&n, 0.999));
    }
  }
  
  nlFree(&n);

  return 0;
}
  

  
