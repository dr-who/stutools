#define _XOPEN_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <assert.h>


float *setup(char *fn, int maxdays, int verbose) {

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

void dump(const char *fn, const float *days, const int maxdays) {
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



void days(char *d, const int num) {
  for (size_t i = 0; i <num; i++) {
    d[i] = '.';
  }
}

int simulate(char *days, float *f, const int num) {
  int alive = 1;
  int deathday = 0;
  
  for (size_t i = 0; i < num; i++) {
    float ran = drand48();
    if (!alive || (ran < ((1-f[i])/365.0))) {
      if (alive) deathday = i;
      alive = 0;
      days[i] = 'x';
    }
  }
  return deathday;
}


int cmpfunc (const void * a, const void * b) {
   return ( *(int*)a - *(int*)b );
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


  if (verbose) {
    fprintf(stderr,"*info* stutools: Monte-Carlo failure simulator\n");
    fprintf(stderr,"*info* k %d, m %d, years %.1lf, rebuild threshold %d\n", k, m, years, rebuildthreshold);
  }
  
  srand48(41);
  
  float *f = setup("hdd-surviverates.dat", maxdays, verbose);
  if (dumpprobs) dump(dumpprobs, f, maxdays);
  

  char *d = malloc(maxdays);

  const int drives = k + m;

  int ok = 0, bad = 0;
  
  for (size_t s = 0; s < samples; s++) {
    int death = 0, diskdead[drives];
    
    for (size_t i = 0; i < drives; i++) {
      diskdead[i] = i;
    }

    // simulate each drive
    for (size_t i = 0; i < drives; i++) {
      days(d, maxdays);
      death = simulate(d, f, maxdays);
      diskdead[i] = death;
    }

    qsort(diskdead, drives, sizeof(int), cmpfunc);
    int tdl = 0;
    for (size_t i = 0; i < drives; i++) {
      if (verbose >= 2) {
	fprintf(stderr, "device %2zd fail day: %d\n", i, diskdead[i]);
      }
      if (diskdead[i]) {
	if ((i > m) && (diskdead[i-m])) { // m 2, dead on 3
	  if (diskdead[i] - diskdead[i-m] < rebuildthreshold) {
	    for (size_t j = i-m; j <= i; j++) {
	      if (verbose >= 2) fprintf(stderr,"dates: %d\n", diskdead[j]);
	    }
	    if (verbose) fprintf(stderr,"> %d drives died while rebuilding (year %.1lf) %d and %d\n", m, diskdead[i]/365.0, diskdead[i-m], diskdead[i]);
	    tdl = diskdead[i];
	    break;
	  }
	}
      }
    }
    if (tdl == 0) {
      ok++;
    } else {
      bad++;
    }
    if (verbose) fprintf(stderr, "Arrays that failed at least once in %.1lf years and rebuilt under %d days. Total %d, Failed array, %d, Failed %% %.1lf %% (sample %zd)\n", years, rebuildthreshold, ok, bad, bad*100.0/(ok+bad), s+1);
  }

  fprintf(stdout, "%.1lf\n", bad * 100.0 / (ok+bad));
  if (d) free(d);
  if (f) free(f);

  return 0;
}



