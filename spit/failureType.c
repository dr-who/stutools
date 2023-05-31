
#include <assert.h>
#include <malloc.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <math.h>

#include "failureType.h"
#include "numList.h"

extern int keepRunning;

// -1 16+2
// -2 16+2
// -2 16+2 -g 2

void virtualDeviceInit(virtualDeviceType *f, char *string, int k, int m, int lhs, size_t sizeInBytes, float flatprob) {
  memset(f, 0, sizeof(virtualDeviceType));
  f->name = strdup(string);
  f->k = k;
  f->m = m;
  f->flatProb = flatprob;
  f->LHSmax = lhs; //local hot spares, max
  f->LHS = lhs; //local hot spares, current
  f->total = f->k + f->m;
  f->status = calloc(f->total, sizeof(int));
  f->driveSizeInBytes = sizeInBytes;
  deviceProbInit(&f->dp, 4000);
  deviceProbFlat(&f->dp, flatprob);
  //  fprintf(stderr,"day 0 prob %f\n", f->dp.probs[0]);
}

int virtualDeviceIterate(virtualDeviceType *f, failureGroup *g) {
  //  printf("iterate name: %s\n", f->name);
  int diedage = 0;
  const size_t rebuildDays = ceil((f->driveSizeInBytes / g->rebuildSpeedInBytesPerSec) * 1.0 / (3600*24));
  for (size_t i = 0; i < f->total; i++) { 
    //    printf("\n%f\n", f->dp.probs[f->age]);
    if ((f->status[i] >= 0) && (drand48() < f->dp.probs[f->age])) {
      diedage = f->status[i];
      f->drivesFailed++;
      //      printf("%d\n", (diedage*24)); //died
      if (f->LHS > 0) {
	// if we have a spare ready
	//	fprintf(stderr,"LHS %zd at age %zd\n", f->LHS, f->age);
	f->LHS--;
	f->status[i] = -rebuildDays;
	f->LHSstatus = f->LHSstatus - g->order;
      } else {
	f->status[i] = -(g->order) - rebuildDays;
      }
    } else {
      f->status[i]++;
    }
  }
  
  if (f->LHSstatus < 0) f->LHSstatus++;
  else if (f->LHSstatus == 0) {
    if (f->LHS < f->LHSmax) {
      f->LHS++;
    }
  }
  
  f->age++;
  // check
  size_t fail = 0;
  for (size_t i = 0; i < f->total; i++) {
    if (f->status[i] < 0) {
      fail++;
    }
  }
  if (fail > f->m) {
    return diedage;
  }
  return 0;
}

void virtualDeviceDump(virtualDeviceType *f, failureGroup *g, size_t showDrives) {
  if (showDrives == 0) {
    fprintf(stderr, "*info*   Virtual Device: k=%zd, m=%zd, lhs=%zd\n", f->k, f->m, f->LHS);
    fprintf(stderr, "*info*       Drive size: %.0lf (TB)\n", ceil(f->driveSizeInBytes / 1024.0/1024/1024/1024));
    fprintf(stderr, "*info*       Drive survival probability/year: %.2lf\n", f->flatProb);
    const size_t rebuildDays = ceil((f->driveSizeInBytes / g->rebuildSpeedInBytesPerSec) * 1.0 / (3600*24));
    fprintf(stderr, "*info*       Rebuild speed %.0lf MB/s (%zd days)\n", ceil(g->rebuildSpeedInBytesPerSec / 1024.0 /1024), rebuildDays);
  }

  if (showDrives) {
    fprintf(stderr, "(%s LHS=%zd/%d) ", f->name, f->LHS, f->LHSstatus);
    size_t fail = 0;
    for (size_t i = 0; i < f->total; i++) {
      fprintf(stderr, "%3d ", f->status[i]);
      if (f->status[i] < 0) {
	fail++;
      }
    }
    fprintf(stderr, " ==> %zd\n", fail);
  } //
}  

void virtualDeviceFree(virtualDeviceType *g) {
  free(g->name);
  free(g->status);
  free(g->probs);
}


// [16+2] [16+2] -g 2
void failureGroupInit(failureGroup *g, int ghs, int order, size_t rebuildSpeed) {
  memset(g, 0, sizeof(failureGroup));
  g->len = 0;
  g->f = NULL;
  g->GHS = ghs;
  g->order = order;
  g->rebuildSpeedInBytesPerSec = rebuildSpeed;
}


void failureGroupDump(failureGroup *g, size_t showDrives) {
  if (showDrives == 0) {
    fprintf(stderr, "*info* Group of %zd virtualDevices\n", g->len);
    fprintf(stderr, "*info*     Global hot spares: %zd\n", g->GHS);
    fprintf(stderr, "*info*     Time for replacements: %zd (days)\n", g->order);
  }
  for (size_t i =0; i < g->len; i++) {
    virtualDeviceDump(&g->f[i], g, showDrives);
  }
}


void failureGroupFree(failureGroup *g) {
  for (size_t i = 0; i < g->len; i++){
    virtualDeviceFree(&g->f[i]);
  }
  free(g->f);
}

void failureGroupAdd(failureGroup *g, virtualDeviceType *f) {
  g->len++;
  g->f = realloc(g->f, g->len * sizeof(virtualDeviceType));
  g->f[g->len - 1] = *f;
}

int failureGroupIterate(failureGroup *g) {
  size_t fails = 0, age = 0;
  for (size_t i = 0; i <g->len; i++) {
    age = virtualDeviceIterate(&(g->f[i]), g);
    if (age != 0) {
      fails++;
    }
  }
  if (fails >= g->len) {
    return age;
  }
  return 0;
}
			 
int failureType(int argc, char *argv[]) {
  srand48(time(NULL));

  if (argc <4) {
    return 1;
  }

  int k = atoi(argv[1]);
  int m = atoi(argv[2]);
  int lhs = atoi(argv[3]);
  const size_t years = 5;

  fprintf(stderr,"*info* Simulation for %zd years\n", years);
  
  for (size_t p = 1; p<=2; p++) {
    size_t good =0, bad = 0;
    fprintf(stderr,"*** Number of Failure Groups: %zd (%s)\n", p, (p==1)?"Single Server":"Mirror");
    
    numListType failAge, sparesRequired;
    nlInit(&failAge, 1000000);
    nlInit(&sparesRequired, 1000000);
    
    for (size_t i = 0 ; keepRunning && (i < 10000); i++) {
      //    printf("--> %zd\n", i);
      failureGroup g;
      
      failureGroupInit(&g, 0, 60, 20*1024*1024);
      
      virtualDeviceType f;
      
      virtualDeviceInit(&f, "a", k, m, lhs, 20L*1024*1024*1024*1024, 0.95);
      failureGroupAdd(&g, &f);
      
      virtualDeviceType f2;
      if (p==2) {
	virtualDeviceInit(&f2, "b", k, m, lhs, 20L*1024*1024*1024*1024, 0.95);
	failureGroupAdd(&g, &f2);
      }
      
      // all setup
      if (i==0) {
	// just dump the first one
	failureGroupDump(&g, 0);
      }
      
      
      int thisonebad = 0;
      for (size_t i = 0; i < 365 * years; i++) {
	int age = failureGroupIterate(&g);
	if (age != 0) {
	  nlAdd(&failAge, age * 24); // failure

	  thisonebad = 1;
	  break;
	}
	//	failureGroupDump(&g, 1);
      }

      // count drives that have failed in this array
      for (size_t k = 0; k < g.len; k++) {
	nlAdd(&sparesRequired, g.f[k].drivesFailed);
      }	    
	  
      
      
      if (thisonebad) {
	bad++;
      } else {
	good++;
      }
      
      
      failureGroupFree(&g);
    }
    fprintf(stderr,"*** Array failure statistics\n");
    if (nlN(&failAge) > 0) {
      fprintf(stderr,"Array failure statistics (failure hour) \n");
      nlBriefSummary(&failAge);
    } else {
      fprintf(stderr,"There were *no* simulated array failures\n");
    }
    fprintf(stderr,"*** Spares required (drives)\n");
    nlBriefSummary(&sparesRequired);
    
    nlFree(&failAge);
    nlFree(&sparesRequired);

    fprintf(stderr,"*** Simulation summary\n");
    fprintf(stderr, "Arrays: %zd good (%.3lf %%), %zd bad (%.3lf)\n", good, good * 100.0 / (good + bad), bad, bad * 100.0 / (good + bad));
    if (p == 1) {
      fprintf(stderr, "\n");
    }
  }

  return 0;
}
