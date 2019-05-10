#include <stdio.h>
#include <stdlib.h>

#include <malloc.h>
#include <limits.h>

typedef struct {
  size_t num;
  size_t sorted;
  double *values;
} numListType;

void nlInit(numListType *n) {
  n->num = 0;
  n->values = NULL;
  n->sorted = 0;
}

void nlFree(numListType *n) {
  if (n->values) free(n->values);
  nlInit(n);
}

   

size_t nlAdd(numListType *n, double value) {
  n->values = realloc(n->values, (n->num+1) * sizeof(double));
  n->values[n->num] = value;
  n->sorted = 0;
  n->num++;
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

double nlMedian(numListType *n) {
  nlSort(n);
  if (n->num > 0) {
    if ((n->num % 2) == 0) {
      // even
      const size_t i = n->num / 2;
      return (n->values[i] + n->values[i-1]) / 2.0;
    } else {
      // odd
      return n->values[n->num / 2];
    }
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
    nlAdd(&n, v);
  }

  if (n.num > 0) {
    fprintf(stdout,"%g\n", nlMedian(&n));
  }
  
  nlFree(&n);

  return 0;
}
  

  
