#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <malloc.h>
#include <limits.h>
#include <assert.h>
#include <string.h>

typedef struct {
  double value;
  size_t age;
} pointType;

typedef struct {
  size_t num;
  size_t sorted;
  pointType *values;
  size_t ever, window;
  double sum;
} numListType;

void nlInit(numListType *n, int window) {
  if (window < 0) window = 1;
  n->num = 0;
  n->values = NULL;
  n->sorted = 0;
  n->ever = 0;
  n->window = window;
  n->sum = 0;
}

void nlFree(numListType *n) {
  if (n->values) free(n->values);
  nlInit(n, n->window);
}

   

size_t nlAdd(numListType *n, double value) {

  n->sum += value;
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

double nlSum(numListType *n) {
  return n->sum;
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
    return 0.0/0.0;
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


void nlDump(numListType *n) {
  for (size_t i = 0; i < n->num; i++) {
    fprintf(stdout,"%g (age %zd)\n", n->values[i].value, n->values[i].age);
  }
}


void usage(int averagedefault) {
  fprintf(stderr,"Usage:\n  dist [-a window] (in/out from stdin/stdout)\n");
  fprintf(stderr,"\nExamples:\n");
  fprintf(stderr,"  dist        # defaults\n");
  fprintf(stderr,"  dist -c n   # use column n. Default to column 1\n");
  fprintf(stderr,"  dist -n     # no header\n");
  fprintf(stderr,"  dist -m     # add a mean value\n");
  fprintf(stderr,"  dist -s     # add a triangular smoothed value\n");
  fprintf(stderr,"  dist -a 2   # window set to 2 samples. Default %d\n", averagedefault);
  fprintf(stderr,"  dist -i n   # ignore first n samples\n");
  fprintf(stderr,"  dist -l     # add line/sample value\n");
  fprintf(stderr,"  dist -p n   # prefix line output with column n\n");
  fprintf(stderr,"  dist -t     # add total/sum value\n");
}


int main(int argc, char *argv[]) {

  int opt;
  const int averagedefault = 30;
  int average = averagedefault, mean = 0;
  size_t header = 0;
  size_t smoothed = 0;
  size_t ignore = 0;
  size_t showline = 0;
  size_t showtotal = 0;
  double scaletotal = 1;
  size_t usecolumn = 1;
  size_t prefixcolumn = 0;
  
  while ((opt = getopt(argc, argv, "a:hmnsi:ltT:c:p:")) != -1) {
    switch(opt) {
    case 'a':
      average = atoi(optarg);
      if (average < 1) average = 1;
      fprintf(stderr,"*info* average window size set to %d\n", average);
      break;
    case 'c':
      usecolumn = atoi(optarg); // starts from 0
      if (usecolumn < 1) usecolumn = 1;
      break;
    case 'n':
      header = 1;
      break;
    case 'h':
      usage(averagedefault);
      exit(1);
      break;
    case 'i':
      ignore = atoi(optarg);
      fprintf(stderr,"*info* ignoring first %zd samples\n", ignore);
      break;
    case 'l':
      showline = 1;
      break;
    case 'p':
      prefixcolumn = atoi(optarg);
      if (prefixcolumn < 1) {
	prefixcolumn = 1;
      }
      break;
    case 's':
      smoothed = 1;
      break;
    case 't':
      showtotal = 1;
      break;
    case 'T':
      scaletotal = atof(optarg);
      fprintf(stderr,"*info* scaling the total value / %.2lf\n", scaletotal);
      if (scaletotal == 0) {
	fprintf(stderr,"*error* -T 0 isn't valid\n");
	exit(1);
      }
      break;
    case 'm':
      mean = 1;
      fprintf(stderr,"*info* appending window mean value\n");
      break;
    default:
      break;
    }
  }
  
  numListType n;
  
  nlInit(&n, average);

  double v = 0;
  size_t sample = 0;


  ssize_t read;
  char * line = NULL;
  size_t len = 0;
  const char tt[2] = " \t";
  while ((read = getline(&line, &len, stdin)) != -1) {
    char *token = strtok(line, tt);
    char *prefix = NULL;
    size_t col = 0;
    while( token != NULL ) {
      col++;
      if (col == prefixcolumn) {
	prefix = token;
      }
      if (col == usecolumn) {
	//	fprintf(stdout," - %s -", token);
	break;
      }
      token = strtok(NULL, tt);
    }
    
    if (token) {
      v = atof(token);
      sample++;
      if (sample <= ignore) continue;
      //    fprintf(stdout,"*info* in %g\n", v);
      nlAdd(&n, v);
      if (n.num > 0) {
	if (!header) {
	  header=1;
	  if (prefixcolumn) {
	    fprintf(stdout,"col%zd\t", prefixcolumn);
	  }
	  fprintf(stdout,"%s'0%%'\t'0.15%%'\t'2.5%%'\t'16%%'\t'50%%'\t'84%%'\t'97.5%%'\t'99.85%%'\t'100%%'%s%s%s\n", (showline==0)?"":"line\t", (mean==0)?"":"\tmean", (smoothed==0)?"":"\tsmoothed", (showtotal==0)?"":"\ttotal");
	}
	if (showline) fprintf(stdout,"%zd\t", sample);
	if (prefix) fprintf(stdout,"%s\t", prefix);
	fprintf(stdout,"%g\t%g\t%g\t%g\t%g\t%g\t%g\t%g\t%g", nlSortedPos(&n, 0), nlSortedPos(&n, 0.015), nlSortedPos(&n, 0.025), nlSortedPos(&n, 0.16), nlMedian(&n), nlSortedPos(&n, 0.84), nlSortedPos(&n, 0.975), nlSortedPos(&n, 0.9985), nlSortedPos(&n, 1));
	if (smoothed) fprintf(stdout, "\t%g", nlSortedSmoothed(&n));
	if (mean) fprintf(stdout,"\t%g", nlMean(&n));
	if (showtotal) fprintf(stdout,"\t%lf", nlSum(&n) / scaletotal);
	fprintf(stdout, "\n");
      }
    }
  }
  
  nlFree(&n);

  return 0;
}
  

  
