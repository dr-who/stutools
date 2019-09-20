#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <malloc.h>
#include <limits.h>
#include <assert.h>

typedef struct {
  double value;
  size_t age;
} pointType;

typedef struct {
  size_t num;
  size_t sorted;
  pointType *values;
  size_t ever, window;
} numListType;

void nlInit(numListType *n, int window) {
  if (window < 0) window = 1;
  n->num = 0;
  n->values = NULL;
  n->sorted = 0;
  n->ever = 0;
  n->window = window;
}

void nlFree(numListType *n) {
  if (n->values) free(n->values);
  nlInit(n, n->window);
}

   

size_t nlAdd(numListType *n, double value) {

  size_t addat = n->num;

  if (n->num < n->window ) {
    n->values = realloc(n->values, (n->num+1) * sizeof(pointType));
    n->num++;
  } else {
    size_t lowest = n->ever + 1;
    for (size_t i = 0; i < n->num; i++) {
      //      fprintf(stdout,"*checking* %zd, value %g age %zd\n", i, n->values[i].value, n->values[i].age);
      if (n->values[i].age < lowest) {
	lowest = n->values[i].age;
	addat = i;
      }
    }
    //    fprintf(stdout,"replacing oldest at position %zd\n", addat);
  }

  n->values[addat].value = value;
  n->values[addat].age = n->ever++;
  n->sorted = 0;

    //
  return n->num;
}


static int nl_sortfunction(const void *origp1, const void *origp2)
{
  pointType *p1 = (pointType*)origp1;
  pointType *p2 = (pointType*)origp2;
  
  if (p1->value < p2->value) return -1;
  else if (p1->value > p2->value) return 1;
  else return 0;
}


void nlSort(numListType *n) {
  if (!n->sorted) {
    qsort(n->values, n->num, sizeof(pointType), nl_sortfunction);
    n->sorted = 1;
  }
}

double nlSortedPos(numListType *n, double pos) {
  nlSort(n);
  assert(pos>=0);
  assert(pos <= 1);
  size_t i = (size_t) ((n->num-1) * pos + 0.5);

  //    fprintf(stdout,"*info* index %zd (%zd and %lf)\n", i, n->num, pos);
    /*  if ((i % 2) == 1) {
    return (n->values[i].value + n->values[i+1].value) / 2.0;
  } else {
  // odd*/
    return n->values[i].value;
    //  }
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
    fprintf(stdout,"%g (age %zd)\n", n->values[i].value, n->values[i].age);
  }
}


void usage(int averagedefault) {
  fprintf(stderr,"Usage:\n  median [-a window] (in/out from stdin/stdout)\n");
  fprintf(stderr,"\nExamples:\n");
  fprintf(stderr,"  median        # defaults to %d samples\n", averagedefault);
  fprintf(stderr,"  median -a 2   # window set to 2 samples\n");
}


int main(int argc, char *argv[]) {

  int opt;
  const int averagedefault = 60;
  int average = averagedefault;
  
  while ((opt = getopt(argc, argv, "a:h")) != -1) {
    switch(opt) {
    case 'a':
      average = atoi(optarg);
      if (average < 1) average = 1;
      fprintf(stderr,"*info* average window size set to %d\n", average);
      break;
    case 'h':
      usage(averagedefault);
      exit(1);
      break;
    default:
      break;
    }
  }
  
  numListType n;
  
  nlInit(&n, average);

  double v = 0;
  while (scanf("%lf", &v) == 1) {
    //    fprintf(stdout,"*info* in %g\n", v);
    nlAdd(&n, v);
    if (n.num > 0) {
      if (n.num > 0) {
	fprintf(stdout,"%g\n", nlMedian(&n));
      }
    }
  }
  
  nlFree(&n);

  return 0;
}
  

  
