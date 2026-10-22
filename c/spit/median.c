#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <getopt.h>

#include "numList.h"


typedef struct {
  numListType list;
  double key;
} pairType;

int find(double key, pairType *numbers, size_t numcount) {
  if (numbers) {
    for (size_t i = 0; i < numcount; i++) {
      if (key == numbers[i].key) {
	return i;
      }
    }
  }
  return -1; // -1 is not found
}

pairType* addKey(double key, pairType *numbers, size_t *numcount) {
  pairType *p = numbers;

  size_t s = *numcount;
  
  p = realloc(p, sizeof(pairType) * (s+1));
  nlInit(&p[s].list, 100000);
  p[s].key = key;

  *numcount = (*numcount) + 1;
  return p;
}

pairType *addValues(double key, pairType *numbers, size_t *numcount, double val){
  int f = find(key, numbers, *numcount);
  if (f == -1) {
    numbers = addKey(key, numbers, numcount);
    f = *numcount - 1;
    // added
  }
  nlAdd(&numbers[f].list, val);

  return numbers;
}



int main(int argc, char *argv[]) {

  int opt;
  size_t modulo = 0;
  
  const char *getoptstring = "m:";
  while ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'm':
      modulo = atoi(optarg);
      fprintf(stderr,"*info* performing modulo %zd\n", modulo);
      break;
    default:
      break;
    }
  }
  
  
  pairType *numbers = NULL;
  size_t numcount = 0;

  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;

  while ((read = getline(&line, &len, stdin)) != -1) {
    double k;
    double v;
    int s = sscanf(line, "%lf %lf\n", &k, &v);
    if (s == 2) {
      numbers = addValues(k, numbers, &numcount, v);
      //      fprintf(stderr,"adding key %lf, value %lf\n", k, v);
    }
  }
  free(line);


  for (size_t i = 0; i < numcount; i++) {
    double themin = nlSortedPos(&numbers[i].list, 0);
    double themax = nlSortedPos(&numbers[i].list, 1);
    const size_t N = nlN(&numbers[i].list);
    double median = 0;

    numListType *p = NULL;
    
    fprintf(stdout,"%8.0lf\t", numbers[i].key);

    if (modulo && (themax - themin > (modulo/2))) {
      numListType better;
      nlInit(&better, 100000);
      for (size_t j = 0; j < N; j++) {
	if (numbers[i].list.values[j].value < (modulo/4)) {
	  nlAdd(&better, modulo + numbers[i].list.values[j].value);
	} else {
	  nlAdd(&better, numbers[i].list.values[j].value);
	}
      }
      median = fmod(nlMedian(&better), modulo);
      themin = nlSortedPos(&better, 0);
      themax = nlSortedPos(&better, 1);
      p = &better;
    } else {
      median = nlMedian(&numbers[i].list);
      if (modulo) median = fmod(median, modulo);
      p = &numbers[i].list;
    }
    
    fprintf(stdout,"%lf\t% zd\t%lf\t|", median, N, nlSD(p));
    for (size_t j = 0; j < N; j++) {
      size_t count = 1;
      for (size_t j2 = j+1; j2 < N ; j2++) {
	if (p->values[j].value == p->values[j2].value) {
	  count++;
	}
      }
      fprintf(stdout, "\t%lf(%zd)", p->values[j].value, count);
      j += (count-1);
    }
    fprintf(stdout,"\n");

  }
  return 0;
}
    
