#define _XOPEN_SOURCE 500

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <assert.h>
#include <string.h>

#include "deviceProbs.h"


/**
 * Array Monte-Carlo simulator.
 *
 * Simulation occurs at the per drive per day level. So each simulation
 * is a matrix of d days by n drives.
 * 
 * Probability estimates are read from a estimation table/file.
 */


typedef struct {
  float *probs;
  int maxdays;
  char *fn;
} driveType;

typedef struct {
  int k;
  int m;
  driveType *drive;
  int *alive;
  int setAge;
} raidSetType;

typedef struct {
  int n;
  raidSetType **sets;
  int span;            // span =1 followed by span=0 is MIRROR
  int globalHotSpares; // e.g. 2 global hot spares
  int timeToProvision; // e.g. 5 days to get a new drive
  int timeToRebuild;   // e.g. 14 days rebuild
  driveType *globalDrive;
} vDevType;  


driveType* driveInit(char *fn) {
  driveType *a = calloc(1, sizeof(driveType)); assert(a);

  a->fn = strdup(fn);
  a->maxdays = 3000;
  a->probs = setupProbs(a->fn, a->maxdays, 0);
  return a;
}

void driveShowConfig(driveType *d) {
  fprintf(stderr,"(drive=%s, drivedays=%d)", d->fn, d->maxdays);
}


vDevType* vDevInit(int globalHotSpares, driveType *drive) {
  vDevType *v = calloc(1, sizeof(vDevType)); assert(v);

  v->globalHotSpares = globalHotSpares;
  v->globalDrive = drive;
  
  return v;
}

void vDevAdd(vDevType *v, raidSetType *r) {
  for (size_t i = 0; i < v->n; i++) {
    assert(r != v->sets[i]);
  }
  v->sets = realloc(v->sets, (v->n + 1) * sizeof(raidSetType*));
  v->sets[v->n] = r;
  v->n++;
}


void raidSetShowConfig(raidSetType *r) {
  fprintf(stderr, "  raidSet k=%d, m=%d, ", r->k, r->m);
  driveShowConfig(r->drive);
  fprintf(stderr,"\n");
}

void vDevShowConfig(vDevType *v) {
  fprintf(stderr,"subRaidSets=%d type=%d hot=%d timeToProvision=%d timeToRebuild=%d ", v->n, v->span, v->globalHotSpares, v->timeToProvision, v->timeToRebuild);
  driveShowConfig(v->globalDrive);
  fprintf(stderr,"\n");
  for (int i = 0; i < v->n; i++) {
    raidSetShowConfig(v->sets[i]);
  }
}

void vDevShowBrief(vDevType *v) {
  if(v->span == 0) {
    fprintf(stderr,"mirror - %dx(%d+%d)", v->n, v->sets[0]->k, v->sets[0]->m);
  } else {
    fprintf(stderr,"raid - %d*(%d+%d)", v->n, v->sets[0]->k, v->sets[0]->m);
  }
}



raidSetType* raidSetInit(int k, int m, driveType *drive) {
  raidSetType *r = calloc(1, sizeof(raidSetType)); assert(r);

  r->k = k;
  r->m = m;
  r->drive = drive;

  r->alive = (int*)calloc((r->k + r->m), sizeof(int));
  assert(r->alive);

  r->setAge = 0;

  return r;
}


void raidSetReset(raidSetType *r) {
  for (size_t i = 0; i < r->k + r->m; i++) {
    r->alive[i] = 0;
  }
  r->setAge = 0;
}

int raidSetSimulate(raidSetType *r, int timeToProvision, int timeToRebuild) {
  if (r->setAge >= r->drive->maxdays)
    return 0;

  float *f = r->drive->probs;
  
  const double thres = (1.0 - f[r->setAge]) / 365.0;
  //  fprintf(stderr,"age %3d (%d+%d) %g: ", r->setAge, r->k, r->m, thres);

  r->setAge++;

  int died = 0;
  //  fprintf(stderr,"-->%d\n", r->k + r->m);
  for (size_t i = 0; i < (r->k + r->m); i++) {
    if (drand48() < thres) {
      //      fprintf(stderr,"died at age %d\n", r->alive[i]);
      r->alive[i] = -(timeToProvision + timeToRebuild);
    } else {
      r->alive[i]++;
    }
    if (r->alive[i] < 0) {
      died++;
    }
    //    fprintf(stderr,"%d ", r->alive[i]);
  }
  //  fprintf(stderr,"-> died %d\n", died);
  return died;
}
  


vDevType* setupRAID60() {
  driveType *HDD = driveInit("hdd-goodrates.dat");

  vDevType *v = vDevInit(4, HDD); // 4 global hots
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  vDevAdd(v, raidSetInit(18,2, HDD));
  v->span = 1;
  v->timeToProvision = 0;
  v->timeToRebuild = 7;

  return v;
}


vDevType* setupHiRAID(int spans, int k, int m, int rebuilddays, int span) {
  driveType *HDD = driveInit("hdd-surviverates.dat");

  vDevType *v = vDevInit(0, HDD); // 4 global hots
  for (size_t i = 0; i < spans;i++) {
    vDevAdd(v, raidSetInit(k, m, HDD));
  }
  v->span = span;
  v->timeToProvision = 0;
  v->timeToRebuild = rebuilddays;

  return v;
}

void simulate(vDevType *v, int years, int iterations, int quiet) {
  srand48(time(NULL));
    
  if (!quiet) vDevShowConfig(v);

  int ok = 0, bad = 0;
  for (size_t s = 0; s < iterations; s++) {

    for (size_t i = 0; i < v->n; i++) {
      raidSetReset(v->sets[i]); // reset the simulation
    }

    for (size_t n = 0; n < years * 365; n++) { 
      int spansdied = 0;
      for (size_t i = 0; i < v->n; i++) {
	int died = raidSetSimulate(v->sets[i], v->timeToProvision, v->timeToRebuild);
	if (died > v->sets[i]->m) {
	  spansdied++;
	}
      }
      if (spansdied) {
	if (v->span == 0) { // mirror
	  if (spansdied == v->n) { // if all died then bad
	    bad++;
	    goto label;
	  }
	} else { // if span (not mirror) then any is bad
	  bad++;
	  goto label;
	}
      }
    }
    ok++;
  label:
    {}

    if (!quiet) {
      vDevShowBrief(v);
      fprintf(stderr," ok %d, bad %d, prob of array failure %.3lf%%\n", ok, bad, bad*100.0 / (bad+ok));
    }
  }
  
  if (quiet) {
    vDevShowBrief(v);
    fprintf(stderr," ok %d, bad %d, prob of array failure %.3lf%%\n", ok, bad, bad*100.0 / (bad+ok));
  }
}

void usage(int years, int rebuild, int samples) {
  fprintf(stderr,"usage: ./raidsimulation -s spans -k datadrives -m parity [options]\n");
  fprintf(stderr,"\ndescription:\n");
  fprintf(stderr,"   Monte-Carlo simulation of array failure given drive survival/failure\n");
  fprintf(stderr,"   probabilities\n");
  fprintf(stderr,"\noptions:\n");
  fprintf(stderr,"   -s spans (default 1)\n");
  fprintf(stderr,"   -k data\n");
  fprintf(stderr,"   -m parity\n");
  fprintf(stderr,"   -i iterations (default %d)\n", samples);
  fprintf(stderr,"   -M mirrored array\n");
  fprintf(stderr,"   -r rebuild days (default %d)\n", rebuild);
  fprintf(stderr,"   -y years (default %d)\n", years);
  fprintf(stderr,"   -q quiet\n");
  fprintf(stderr,"\nexamples:\n");
  fprintf(stderr,"   ./raidsimulation -k 182 -m 10 -y 5\n");
  fprintf(stderr,"   ./raidsimulation -M -s 2 -k 8 -m 2 -y 5\n");
}


int main(int argc, char *argv[]) {
  
  optind = 0;
  int spans = 1, k = 0, m = 0, rebuilddays = 14, opt = 0, iterations = 10000;
  int spantype = 1;
  double years = 5;
  int quiet = 0;
  
  while ((opt = getopt(argc, argv, "k:m:y:r:vs:o:p:f:t:i:h:aMq")) != -1) {
    switch(opt) {
    case 'k':
      k = atoi(optarg);
      break;
    case 'm':
      m = atoi(optarg);
      break;
    case 'M':
      spantype = 0;
      break;
    case 's':
      spans = atoi(optarg);
      break;
    case 'i':
      iterations = atoi(optarg);
      break;
    case 'y':
      years = atof(optarg);
      break;
    case 'r':
      rebuilddays = atoi(optarg);
      break;
    case 'q':
      quiet = 1;
      break;
    case 'h':
    default:
      usage(years, rebuilddays, iterations);
      exit(1);
    }
  }

  if (k<1 || m<1) {
    usage(years, rebuilddays, iterations);
    exit(1);
  }
    

  
  vDevType *v = setupHiRAID(spans, k, m, rebuilddays, spantype);
  //  vDevType *v = setupRAID60();

  simulate(v, years, iterations, quiet);

  return 0;


}
