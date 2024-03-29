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
  size_t n;
  int *drive;
  int *daylastreplaced;
} arrayDayType;

typedef struct {
  size_t numDays;
  arrayDayType *day;
  size_t *failedOnDay;
} arrayLifeType;
  


arrayLifeType setupArray(const int days, const int n) {
  arrayLifeType a;
  a.numDays = days;
  a.day = calloc(days, sizeof(arrayDayType));
  a.failedOnDay = calloc(days, sizeof(size_t));
  for (size_t i = 0; i < days; i++) {
    a.day[i].n = n;
    a.day[i].drive = calloc(n, sizeof(int));
    a.day[i].daylastreplaced = calloc(n, sizeof(int));
  }
  
  
  return a;
}

void dumpArrayPPM(const arrayLifeType *a, const int days, const int n, const int m) {

  FILE *fp = fopen("image.ppm", "wt");
  if (fp == NULL) {
    perror("image.ppm");
    return;
  }

  fprintf(fp, "P3\n%d %d\n%d\n", n, days, 255);
  
  for (int j = 0; j < days; j++) {
    // row per day
    int bg = 0; // white
    if ((j==0) && (a->failedOnDay[j])) {
      bg = 1; // yellow
    } else if ((j>0) && (a->failedOnDay[j] > a->failedOnDay[j-1])) {
      bg = 1;
    }
    
    
    for (int i = 0; i < n; i++) {
      int r,g,b;
      int f = a->failedOnDay[j];
      
      if (bg == 0) {r=0;g=0;b=0;}
      else {r = 255 -f; g=255;b=0;}

      if (a->day[j].drive[i]) {
	r=255 - f;g=r; b=r;
	if (a->failedOnDay[j] > m) {
	  r=255; g=0; b=0;
	}
      }
      fprintf(fp," %3d %3d %3d   ", r,g,b);
    }
    fprintf(fp,"\n");
  }
  fclose(fp);
}


void saveArray(const arrayLifeType *a, const int days, const int n, const int m, const int sample) {
  
  char s[100];
  sprintf(s,"_out.round.%04d", sample);
  FILE *fp = fopen(s, "wt");
  for (int j = 0; j < days; j++) {
    fprintf(fp,"%zd\n", a->failedOnDay[j]);
  }
  fclose(fp);
}



void clearArray(arrayLifeType *a, const int days, const int n) {
  for (int j = 0; j < days; j++) {
    a->failedOnDay[j] = 0;
    for (int i = 0; i < n; i++) {
      a->day[j].drive[i] = 0;
      a->day[j].daylastreplaced[i] = 0;
    }
  }
}

void freeArray(arrayLifeType *a, const int days, const int n) {
  for (int j = 0; j < days; j++) {
    free(a->day[j].drive);
    free(a->day[j].daylastreplaced);
  }
  free(a->failedOnDay);
  free(a->day);
}
      


int simulateArray(arrayLifeType *a, float *f, const int days, const int n, const int rebuild, const int m, const int verbose, int *daysrebuilding, int *maxfailed, int *ageofdeath, int *drivesneeded, const int printarray) {

  *drivesneeded = n;
  int notfailedout = 1;

  for (int j = 0; j < days; j++) {
    if (printarray) {
      fprintf(stdout, "%d|", j);
    }
    for (int i = 0; i < n; i++) {
      double ran = drand48();    // return random number between 0 and 1
      assert(ran != 1.0);
      int prev = 0;
      if (printarray) {
	fprintf(stdout, "%d:", a->day[j].daylastreplaced[i]);
      }
      
      if (j>0) prev = a->day[j-1].drive[i];
      
      const double thres = (1.0 - f[j - a->day[j].daylastreplaced[i]]) / 365.0;
      //      fprintf(stderr,"%lf %lf  %f\n", ran, thres, f[j - a->day[j].daylastreplaced[i]]);
      if ((prev>0) || (ran < thres)) {

	a->day[j].drive[i] = prev+1;
	a->failedOnDay[j]++;
	if (printarray) { fprintf(stdout, "fail %d ",a->day[j].drive[i]); }

	notfailedout = 0;
 
	if (a->day[j].drive[i] >= rebuild) {
	  a->day[j].drive[i] = -1;
	  a->failedOnDay[j]--;
	  /* Record that this drive is now replaced, so new drive will
	  have lower chance of failing compared to peers, setting
	  daylastreplaced to current day for this drive for remainder
	  of this simulation run */
	  for ( int c = j; c < days; c++ ) {
	    a->day[c].daylastreplaced[i] = j;
	  }
	}
      }
        
      if (notfailedout && printarray) {
	fprintf(stdout, "%.2f ", f[j - a->day[j].daylastreplaced[i]]);
      }      
      notfailedout = 1;
      
      if (a->day[j].drive[i] == 1) {
	(*drivesneeded)++;
      }
      
    }
     if (printarray) {
       fprintf(stdout, "\n");
     }

  }
  int death = 0;
  *daysrebuilding = 0;
  *maxfailed = 0;
  int founddeath = 0;
  *ageofdeath = 0;
  
  for (int j = 0; j < days; j++) {
    if (a->failedOnDay[j]) {
      (*daysrebuilding)++;
      if (a->failedOnDay[j] > *maxfailed) {
	*maxfailed = a->failedOnDay[j];
      }
      
      if (a->failedOnDay[j] > m) {
	if (verbose > 0) {
	  fprintf(stderr,"day %d, failed drives %zd\n", j, a->failedOnDay[j]);
	}
	death++;

	if (founddeath ==0) {
	  *ageofdeath = j;
	  //	  fprintf(stderr,"dead %d\n", j);
	  founddeath = 1;
	}
      }
    }
  }
  if (verbose) {
    fprintf(stderr,"*info* days %d, daysrebuilding days %d (%.1lf%%), death %d, max failed %d, drives needed %d\n", days, *daysrebuilding, 100.0*(*daysrebuilding)/days, death, *maxfailed, *drivesneeded);
  }
  return death;
}


void usage(int years, int rebuild, int samples) {
  fprintf(stderr,"usage: ./raidfailures -k k -m m [options]\n");
  fprintf(stderr,"\ndescription:\n");
  fprintf(stderr,"   Monte-Carlo simulation of array failure given drive survival/failure\n");
  fprintf(stderr,"   probabilities\n");
  fprintf(stderr,"\noptions:\n");
  fprintf(stderr,"   -t drives\n");
  fprintf(stderr,"   -y years(%d)\n", years);
  fprintf(stderr,"   -r rebuilddays(%d)\n", rebuild);
  fprintf(stderr,"   -s samples(%d)\n", samples);
  fprintf(stderr,"   -i hdd-surviverates.dat        # input day/survival file\n");
  fprintf(stderr,"   -o outprobs.txt\n");
  fprintf(stderr,"   -p probability                 # flat survivial prob over life\n");
  fprintf(stderr,"   -v (verbose)\n");
  fprintf(stderr,"   -a print array - use with small values of s only!\n");
  fprintf(stderr,"\ngraphics:\n");
  fprintf(stderr,"   pnmtopng image.ppm >image.png  # generates a day vs disk image\n");
}



int main(int argc, char *argv[]) {

  optind = 0;
  int k = 0, m = 0, t = 0, rebuilddays = 7, opt = 0, verbose = 0, samples = 10000, printarray = 0;
  char *dumpprobs = NULL;
  double years = 5;
  char *inname = NULL;
  float flatrate = 0;
  
  while ((opt = getopt(argc, argv, "k:m:y:r:vs:o:p:f:t:i:h:a")) != -1) {
    switch(opt) {
    case 'k':
      k = atoi(optarg);
      break;
    case 'm':
      m = atoi(optarg);
      break;
    case 'i':
      inname = strdup(optarg);
      break;
    case 'y':
      years = atof(optarg);
      break;
    case 't':
      t = atoi(optarg);
      break;
    case 'r':
      rebuilddays = atoi(optarg);
      break;
    case 'p':
      flatrate = atof(optarg);
      break;
    case 'o':
      dumpprobs = strdup(optarg);
      break;
    case 'a':
      printarray++;
      break;
    case 's':
      samples = atoi(optarg);
      break;
    case 'v':
      verbose++;
      break;
    case 'h':
    default:
      usage(years, rebuilddays, samples);
      exit(1);
    }
  }

  if (inname == NULL) {
    inname = strdup("hdd-surviverates.dat");
  }
  
  int maxdays = years * 365;
  if (rebuilddays == 0) rebuilddays = maxdays;

  if (t > 0) {
    if (k == 0 && m > 0) {
      k = t - m;
    } else if (m == 0 && k > 0) {
      m = t - k;
    }
  } else {
    t = k+m;
  }

  if ((k<1) || (m<0) || (maxdays < k+m)) {
    usage(years, rebuilddays, samples);
    exit(1);
  }



  fprintf(stderr,"*info* stutools: Monte-Carlo k+m/RAID failure simulator (%d samples)\n", samples);
  fprintf(stderr,"*info* total devices %d, k %d, m %d, years %.1lf, rebuild days %d\n", t, k, m, years, rebuilddays);
  
  srand48(time(NULL));

  float *f = NULL;
  if (flatrate) {
    fprintf(stderr,"*info* flatrate = %.3lf\n", flatrate);
    f = setupProbsFlat(maxdays, flatrate);
  } else {
    fprintf(stderr,"*info* loading probabilities from '%s'\n", inname);
    f = setupProbs(inname, maxdays, verbose);
  }
  
  if (dumpprobs) {
    if (strcmp(inname, dumpprobs)==0) {
      fprintf(stderr,"*error* don't use -o to clobber the input file\n");
      exit(1);
    }
    fprintf(stderr,"*info* output per day probss: %s\n", dumpprobs);
    dumpProbs(dumpprobs, f, maxdays);
  }


  int ok = 0, bad = 0;
  
  arrayLifeType a = setupArray(maxdays, t);

  int daysrebuilding = 0, daysrebuildingtotal = 0, globalmaxfailed = 0, globalageofdeath = 0, globaldrivesneeded = 0;
  for (size_t s = 0; s < samples; s++) {
    int maxfailed = 0, ageofdeath = 0, drivesneeded = 0;
    clearArray(&a, maxdays, t);
    int death = simulateArray(&a, f, maxdays, t, rebuilddays, m, verbose, &daysrebuilding, &maxfailed, &ageofdeath, &drivesneeded, printarray);
    
    if (death == 0) {
      ok++;
    } else {
      bad++;
    }    

    daysrebuildingtotal += daysrebuilding;

    globaldrivesneeded += drivesneeded;

    if (maxfailed > globalmaxfailed) {
      globalmaxfailed = maxfailed;
    }

    if (ageofdeath > 0) {
      globalageofdeath += ageofdeath;
      //      fprintf(stderr,"age of d %d\n", ageofdeath);
    }

    if (s==0) {
      dumpArrayPPM(&a, maxdays, t, m);
    }
      
    if (verbose > 1) {
      //      dumpArray(&a, maxdays, t, m);
    }
    //        saveArray(&a, maxdays, t, m, s);
    
    if (verbose) fprintf(stderr, "Arrays that failed at least once in %.1lf years and rebuilt under %d maxdays. Total %d, Failed array, %d, Failed %% %.1lf %% (sample %zd)\n", years, rebuilddays, ok, bad, bad*100.0/(ok+bad), s+1);
  }
  freeArray(&a, maxdays, t);


  fprintf(stdout, "%d\t%.3lf %%\t%.3lf %%\t%d maxfail\t%.3lf avgage\t%d drivesneeded\t%d\t%d\n", bad, bad * 100.0 / (ok+bad), 100.0*daysrebuildingtotal / (samples * maxdays), globalmaxfailed, bad==0?0:globalageofdeath * 1.0 / bad, globaldrivesneeded / samples, k, m);
  if (f) free(f);
  if (inname) free(inname);
  if (dumpprobs) free(dumpprobs);

  fprintf(stderr,"\n*info* this package is open source and unvalidated. It probably contains terrible errors.\n");
  return 0;
}




/// virtualDevice vdev;  2x{8+2} is MIRROR, 8*{8+2}+2 (two global hot spares)
/// vdev.type = AND; then two other RAIDset (8,2)
/// vdev.type = AND or OR;
/// array of arrayLifeType. Say 10 of 8+2. Tag each drive with a type. Tag the 10 so it's an AND
/// say mirror 2 x 8+2, so it's an OR
/// or OR simulate each day, if one fails that's fine, if both fails then that's data loss.

/// globalHotSpares, count per
/// time to rebuild

/// construct a bunch of vdevs

/// Init(vdev);
/// OneSample(vdev, 1000 days); // 1,000 days
///    this will iterate through the components of the vdev, each raid set, add a day
///    check with OR and AND
///    Keep track
/// OneSample(vdev, 1000 days); 


