#define _XOPEN_SOURCE 500

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <assert.h>
#include <math.h>
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
  fprintf(stderr,"*info* loaded '%s'\n", fn);
  fprintf(stderr,"*info* day 0 dailySurvivalRate = %.6lf ^ 365 = %.3lf\n", a->probs[0], pow(a->probs[0], 365.0));
  fprintf(stderr,"*info* 1 year dailySurvivalRate = %.6lf ^ 365 = %.3lf\n", a->probs[365], pow(a->probs[365], 365.0));
  fprintf(stderr,"*info* 5 year dailySurvivalRate = %.6lf ^ 365 = %.3lf\n", a->probs[365*5], pow(a->probs[365*5], 365.0));
  return a;
}

driveType* driveInitFlat(char *name, float prob) {
  driveType *a = calloc(1, sizeof(driveType)); assert(a);

  a->fn = strdup(name);
  a->maxdays = 3000;
  a->probs = setupProbsFlat(a->maxdays, prob);
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

int raidSetSimulate(raidSetType *r, int timeToProvision, int timeToRebuild, int print) {
  if (r->setAge >= r->drive->maxdays)
    return 0;

  float *f = r->drive->probs;

  const double thres = 1.0 - f[r->setAge];
  //  fprintf(stderr,"thres = %lf\n", thres);

  r->setAge++;
  //  assert (r->setAge < 4000);

  int died = 0;
  for (size_t i = 0; i < (r->k + r->m); i++) {
    if (drand48() <= thres) {
      r->alive[i] = -(timeToProvision + timeToRebuild);
    } else {
      r->alive[i]++;
    }
    
    if (r->alive[i] < 0) {
      died++;
    }
  }
  if (print) {
    for (size_t i = 0; i < (r->k + r->m); i++) {
      fprintf(stderr,"%3d ", r->alive[i]);
    }
    fprintf(stderr,"-> died %d\n", died);
    }
  return died;
}
  


vDevType* setupRAID60(driveType *drive, int spans, int k, int m, int globalspares, int rebuildtime, int spantype) {

  vDevType *v = vDevInit(globalspares, drive); // 4 global hots
  for (size_t i = 0; i < spans; i++) {
    vDevAdd(v, raidSetInit(k, m, drive));
  }
  v->span = spantype;
  v->timeToProvision = 0;
  v->timeToRebuild = rebuildtime;

  return v;
}


vDevType* setupHiRAID(driveType *drive, int spans, int k, int m, int rebuilddays, int span) {

  vDevType *v = vDevInit(0, drive); // 4 global hots
  for (size_t i = 0; i < spans;i++) {
    vDevAdd(v, raidSetInit(k, m, drive));
  }
  v->span = span;
  v->timeToProvision = 0;
  v->timeToRebuild = rebuilddays;

  return v;
}

void simulate(vDevType **v, int numVDevs, int maxdays, int iterations, int quiet, int print) {
  srand48(time(NULL));
    
  int *ok = calloc(numVDevs, sizeof(int));
  int *bad = calloc(numVDevs, sizeof(int));
  int *raidsetdied = calloc(numVDevs, sizeof(int));

  for (size_t s = 0; s < iterations; s++) {
    //    fprintf(stderr,"iteration %zd\n", 1+s);

    for (size_t n = 0; n < numVDevs; n++) {
      raidsetdied[n] = 0;
      
      for (size_t i = 0; i < v[n]->n; i++) {
	raidSetReset(v[n]->sets[i]); // reset the simulation
      }
    }


    for (size_t day = 0; day < maxdays; day++) {  // day iteration

      for (size_t n = 0; n < numVDevs; n++) {
	
	for (size_t i = 0; i < v[n]->n; i++) {
	  if (print) fprintf(stderr,"[sample %zd][day %zd] ", s, day);
	  int died = raidSetSimulate(v[n]->sets[i], v[n]->timeToProvision, v[n]->timeToRebuild, print);
	  if (died > v[n]->sets[i]->m) {
	    raidsetdied[n]++;
	  }
	}
      }
    }
    
    for (size_t n = 0; n < numVDevs; n++) {
      if (raidsetdied[n]) {
	if (v[n]->span == 0) { // mirror
	  fprintf(stderr,"bad %d\n", raidsetdied[n]);
	  if (raidsetdied[n] == v[n]->n) { // if all died then bad
	    bad[n]++;
	  }
	} else { // if span (not mirror) then any is bad
	  bad[n]++;
	}
      } else {
	ok[n]++;
      }
    } // vDev
    
    if (!quiet) {
      for (size_t n = 0; n < numVDevs; n++) {
	fprintf(stderr,"[%zd] ", n);
	vDevShowBrief(v[n]);
	fprintf(stderr," ok %d, bad %d, prob of array failure %.3lf%%\n", ok[n], bad[n], bad[n]*100.0 / (bad[n]+ok[n]));
      }
    }
  }

  // duplicate of above
  if (quiet) {
    for (size_t n = 0; n < numVDevs; n++) {
      fprintf(stderr,"[%zd] ", n);
      vDevShowBrief(v[n]);
      fprintf(stderr," ok %d, bad %d, prob of array failure %.3lf%%\n", ok[n], bad[n], bad[n]*100.0 / (bad[n]+ok[n]));
    }
  }
}

void usage(int years, int rebuild, int samples, int globalspares) {
  fprintf(stderr,"usage: ./raidsimulation -s spans -k datadrives -m parity [options]\n");
  fprintf(stderr,"\ndescription:\n");
  fprintf(stderr,"   Monte-Carlo simulation of array failure given drive survival/failure\n");
  fprintf(stderr,"   probabilities\n");
  fprintf(stderr,"\noptions:\n");
  fprintf(stderr,"   -s spans (default 1)\n");
  fprintf(stderr,"   -k data\n");
  fprintf(stderr,"   -m parity\n");
  fprintf(stderr,"   -d dump array per day\n");
  fprintf(stderr,"   -i iterations (default %d)\n", samples);
  fprintf(stderr,"   -M mirrored array\n");
  fprintf(stderr,"   -r rebuild days (default %d)\n", rebuild);
  fprintf(stderr,"   -g globalspares  %d)\n", globalspares);
  fprintf(stderr,"   -y years (default %d)\n", years);
  fprintf(stderr,"   -b specifyMTBF (hours)\n");
  fprintf(stderr,"   -p annualSurvivalRate [0, 1], 1-AFR\n");
  fprintf(stderr,"   -D dailySurvivalRate [0, 1]\n");
  fprintf(stderr,"   -q quiet\n");
  fprintf(stderr,"\nexamples:\n");
  fprintf(stderr,"   ./raidsimulation -k 182 -m 10 -b 25000    # 25,000 hour MTBF, AFR=0.3, prob = 0.7\n");
  fprintf(stderr,"   ./raidsimulation -k 182 -m 10 -y 5        # load hdd defaults\n");
  fprintf(stderr,"   ./raidsimulation -M -s 2 -k 8 -m 2 -y 5   # Mirrored, two spans\n");
  fprintf(stderr,"   ./raidsimulation -k 204 -m 0 -b 100000 -q -r7   -> array failure 100%%\n");
  fprintf(stderr,"   ./raidsimulation -k 203 -m 1 -b 100000 -q -r7   -> array failure 100%%\n");
  fprintf(stderr,"   ./raidsimulation -k 202 -m 2 -b 100000 -q -r7   -> array failure 98.52%%\n");
  fprintf(stderr,"   ./raidsimulation -k 201 -m 3 -b 100000 -q -r7   -> array failure 34.8%%\n");
  fprintf(stderr,"   ./raidsimulation -k 200 -m 4 -b 100000 -q -r7   -> array failure 3.64%%\n");
  fprintf(stderr,"   ./raidsimulation -k 194 -m 10 -b 100000 -q -r7  -> array failure 0.00%%\n");
  fprintf(stderr,"   ./raidsimulation -k 194 -m 10 -b 100000 -q -r30 -> array failure 0.02%%\n");
  fprintf(stderr,"   ./raidsimulation -k 194 -m 10 -b 100000 -q -r90 -> array failure 44.7%%\n");
}


int main(int argc, char *argv[]) {
  
  optind = 0;
  int spans = 1, k = 0, m = 0, rebuilddays = 7, opt = 0, iterations = 10000, globalspares = 0;
  int spantype = 1;
  double years = 7;
  int quiet = 0, dumpArray = 0;
  float prob = -1;
  
  while ((opt = getopt(argc, argv, "k:m:y:r:vs:o:p:f:t:i:h:aMqb:dD:")) != -1) {
    switch(opt) {
    case 'b':
      {}
      float mtbf = atof(optarg);
      float afr =  1.0 - exp(-(365*24.0)/mtbf);
      prob = pow(1 - afr, 1/365.0);
      if (prob < 0) prob = 0;
      if (prob > 1) prob = 1;
      fprintf(stderr,"*info* mtbf = %.0lf, dailySurvivalRate = %.6lf ^ 365 = %.3lf\n", mtbf, prob, pow(prob, 365));
      break;
    case 'd':
      dumpArray = 1;
      break;
    case 'D':
      prob = atof(optarg);
      fprintf(stderr,"*info* dailySurvivalRate = %.6lf ^ 365 = %.3lf\n", prob, pow(prob, 365));
      break;
    case 'k':
      k = atoi(optarg);
      break;
    case 'p':
      prob = pow(atof(optarg), 1.0/365.0);
      fprintf(stderr,"*info* dailySurvivalRate = %.6lf ^ 365 = %.3lf\n", prob, pow(prob, 365));
      if (prob < 0) prob = 0;
      if (prob > 1) prob = 1;
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
      if (rebuilddays < 1) rebuilddays = 1;
      break;
    case 'q':
      quiet = 1;
      break;
    case 'g':
      globalspares = atoi(optarg);
      break;
    case 'h':
    default:
      usage(years, rebuilddays, iterations, globalspares);
      exit(1);
    }
  }

  if (k<1) {
    usage(years, rebuilddays, iterations, globalspares);
    exit(1);
  }
    

  assert(spantype >= 0);

  driveType *HDD = NULL;


  if (prob >= 0) {
    HDD = driveInitFlat("prob%", prob);
  } else {
    HDD = driveInit("hdd-surviverates.dat");
  }


  
  vDevType **v = calloc(2, sizeof(vDevType*));
  
  //  v[0] = setupHiRAID(spans, k, m, rebuilddays, spantype);
  v[0] = setupRAID60(HDD, spans, k, m, globalspares, rebuilddays, spantype);
  //  v[1] = setupRAID60(spans, k, m, globalspares, rebuilddays);

  simulate(v, 1, years * 365, iterations, quiet, dumpArray);

  return 0;


}
