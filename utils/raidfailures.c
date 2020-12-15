#define _XOPEN_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <assert.h>


typedef struct {
  size_t n;
  int *drive;
} arrayDayType;

typedef struct {
  size_t numDays;
  arrayDayType *day;
  size_t *failedOnDay;
} arrayLifeType;
  

float *setupProbs(char *fn, int maxdays, int verbose) {

  float *f = calloc(sizeof(float), maxdays);

  FILE *fp = fopen(fn, "rt");
  float a,b,c;
  if (fp) {
    if (verbose) fprintf(stderr,"*info* opening survival rate file '%s'\n", fn);
    while (fscanf(fp, "%f %f %f\n", &a, &b, &c) > 0) {
      if (verbose) fprintf(stderr,"%f %f %f\n", a,b,c);
      for (size_t i = a; i < b; i++) {
	if (i < maxdays) 
	  f[i] = c;
      }
    }
    for (size_t i = b; i < maxdays; i++) {
      if (i < maxdays) 
	f[i] = c;
    }
    fclose(fp);
  }

  for (size_t i = 0; i < maxdays; i++) {
    if (f[i] == 0) {
      abort();
    }
  }

  if (verbose) fprintf(stderr,"The last day has a (yearly) survival rate of %.2lf\n", f[maxdays-1]);
  
  return f;
}

void dumpProbs(const char *fn, const float *days, const int maxdays) {
  FILE *fp = fopen(fn, "wt");
  if (fp) {
    for (size_t i = 0; i < maxdays; i++) {
      fprintf(fp, "%zd\t%f\n", i, days[i]);
    }
    fclose(fp);
  } else {
    perror(fn);
    exit(1);
  }
}


arrayLifeType setupArray(const int days, const int n) {
  arrayLifeType a;
  a.numDays = days;
  a.day = calloc(days, sizeof(arrayDayType));
  a.failedOnDay = calloc(days, sizeof(size_t));
  for (size_t i = 0; i < days; i++) {
    a.day[i].n = n;
    a.day[i].drive = calloc(n, sizeof(int));
  }
  return a;
}

void dumpArray(const arrayLifeType *a, const int days, const int n, const int m) {
  for (int j = 0; j < days; j++) {
    fprintf(stderr,"[%3d] ", j);
    for (int i = 0; i < n; i++) {
      fprintf(stderr," %3d", a->day[j].drive[i]);
    }
    fprintf(stderr,"  --> %3zd", a->failedOnDay[j]);
    if (a->failedOnDay[j] > m) fprintf(stderr," ****** FAILED ******");
    fprintf(stderr,"\n");
  }
}

void clearArray(arrayLifeType *a, const int days, const int n) {
  for (int j = 0; j < days; j++) {
    a->failedOnDay[j] = 0;
    for (int i = 0; i < n; i++) {
      a->day[j].drive[i] = 0;
    }
  }
}

void freeArray(arrayLifeType *a, const int days, const int n) {
  for (int j = 0; j < days; j++) {
    free(a->day[j].drive);
  }
  free(a->failedOnDay);
  free(a->day);
}
      


int simulateArray(arrayLifeType *a, float *f, const int days, const int n, const int rebuild, const int m, const int verbose, int *rebuilding) {
  for (int j = 0; j < days; j++) {
    for (int i = 0; i < n; i++) {
      float ran = drand48();
      int prev = 0;
      if (j>0) prev = a->day[j-1].drive[i];

      if ((prev>0) || (ran < ((1-f[j])/365.0))) {
	a->day[j].drive[i] = prev+1;
	a->failedOnDay[j]++;
	if (a->day[j].drive[i] > rebuild) {
	  a->day[j].drive[i] = 0;
	  a->failedOnDay[j]--;
	}
      }
    }
  }
  int death = 0;
  *rebuilding = 0;
  for (int j = 0; j < days; j++) {
    if (a->failedOnDay[j]) {
      (*rebuilding)++;
      
      if (a->failedOnDay[j] > m) {
	if (verbose > 0) {
	  fprintf(stderr,"day %d, failed drives %zd\n", j, a->failedOnDay[j]);
	}
	death++;
      }
    }
  }
  if (verbose) {
    fprintf(stderr,"*info* days %d, rebuilding days %d (%.1lf%%), death %d\n", days, *rebuilding, 100.0*(*rebuilding)/days, death);
  }
  return death;
}


void usage(int years, int rebuild, int samples) {
  fprintf(stderr,"usage: ./raidfailures -k k -m m [-y years(%d)] [-r rebuilddays(%d)] [-s samples(%d)] [-p prob.txt] [-v (verbose)]\n", years, rebuild, samples);
}


int main(int argc, char *argv[]) {

  optind = 0;
  int k = 0, m = 0, rebuildthreshold = 7, opt = 0, verbose = 0, samples = 1000;
  char *dumpprobs = NULL;
  double years = 5;
  
  while ((opt = getopt(argc, argv, "k:m:y:r:vs:p:")) != -1) {
    switch(opt) {
    case 'k':
      k = atoi(optarg);
      break;
    case 'm':
      m = atoi(optarg);
      break;
    case 'y':
      years = atof(optarg);
      break;
    case 'r':
      rebuildthreshold = atoi(optarg);
      break;
    case 'p':
      dumpprobs = optarg;
      break;
    case 's':
      samples = atoi(optarg);
      break;
    case 'v':
      verbose++;
      break;
    default:
      usage(years, rebuildthreshold, samples);
      exit(1);
    }
  }
  
  int maxdays = years * 365;

  if ((k<1) || (m<1) || (maxdays < k+m)) {
    usage(years, rebuildthreshold, samples);
    exit(1);
  }


  fprintf(stderr,"*info* stutools: Monte-Carlo failure simulator\n");
  fprintf(stderr,"*info* k %d, m %d, years %.1lf, rebuild threshold %d\n", k, m, years, rebuildthreshold);
  
  srand48(41);
  
  float *f = setupProbs("hdd-surviverates.dat", maxdays, verbose);
  if (dumpprobs) dumpProbs(dumpprobs, f, maxdays);

  const int drives = k + m;

  int ok = 0, bad = 0;
  
  arrayLifeType a = setupArray(maxdays, drives);

  int rebuilding = 0;
  for (size_t s = 0; s < samples; s++) {
    clearArray(&a, maxdays, drives);
    int death = simulateArray(&a, f, maxdays, drives, rebuildthreshold, m, verbose, &rebuilding);
    if (verbose > 1) {
      dumpArray(&a, maxdays, drives, m);
    }

    if (death == 0) {
      ok++;
    } else {
      bad++;
    }
    if (verbose) fprintf(stderr, "Arrays that failed at least once in %.1lf years and rebuilt under %d maxdays. Total %d, Failed array, %d, Failed %% %.1lf %% (sample %zd)\n", years, rebuildthreshold, ok, bad, bad*100.0/(ok+bad), s+1);
  }
  freeArray(&a, maxdays, drives);

  fprintf(stdout, "%.1lf\n", bad * 100.0 / (ok+bad));
  if (f) free(f);

  return 0;
}



