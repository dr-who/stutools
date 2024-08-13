#define _XOPEN_SOURCE 500

#include <stdio.h>
#include <time.h>
#include <stdlib.h>
#include <unistd.h>
#include <getopt.h>
#include <assert.h>
#include <string.h>

void usage() {
  printf("usage: stripsim -g 2 -d 13 -k 6 -m 3 -T 20 -s 1\n");
  printf("\n");
  printf("options:\n");
  printf("   -g n    groups of drives\n");
  printf("   -d n    drives per group (total = g x d)\n");
  printf("   -k/m    data/parity\n");
  printf("   -T      size of drive in TB\n");
  printf("   -s      stripe size in MB\n");
  printf("   -H      print histogram\n");
  printf("   -H -H   verbose histogram\n");
  printf("   -f 0,2... list of failed drives\n");
  printf("   -f -5   if negative, fail that many drives\n");
  printf("   -P n    print that many rows of the table\n");
}


int simulate(int *drivesok, int groups, int drives, int k, int m, int T, int s, int Print, int showhist) {
  size_t totaldrives = groups * drives;
  size_t totalrows = T*1000L*1000L / s;

  printf("*info* allocating = %.1lf GB\n", totalrows * totaldrives / 1024.0/1024.0/1024.0);

  unsigned int *a = calloc(totaldrives * totalrows, sizeof(unsigned int));
  if (!a) {
    printf("OOM!, trying to allocate %zd \n", totaldrives * totalrows * sizeof(int));
    exit(1);
  }

  int *stripecounter = calloc(totaldrives, sizeof(int));
  int *shuf = calloc(drives, sizeof(int));


  int maxstrips = T*1000*1000/(s*(k+m));
  //  printf("max strips %d\n", maxstrips);
  int stripid = 0;

  int stripe = 0;
  for (stripe = 0; stripe < maxstrips; stripe++) {
    
    for (int g = 0; g < groups; g++) {
      // setup
      for (int d = 0; d < drives; d++) {
	shuf[d] = g * drives + d;
      }
      // shuffle
      for (int d = 0; d < drives; d++) {
	int i = d;
	int j = drand48() * drives;

	int temp = shuf[i];
	shuf[i] = shuf[j];
	shuf[j] = temp;
	//	printf("drive %d\n", shuf[d]);
      }
      stripid++;
      for (int d = 0; d < k+m; d++) {

	//	printf("%d %zd\n", shuf[d], shuf[d]*totaldrives);
	int ii = stripecounter[shuf[d]] * totaldrives + shuf[d];
	assert(ii >= 0);
	if (ii < totalrows * totaldrives) {
	  a[ii] = stripid; //stripecounter[shuf[d]];
	  
	  stripecounter[shuf[d]]++;
	} else {
	  printf("breaking out!\n");
	  break;
	}
	  
      }
    }


    //    printf("---");
    //    for (int i = 0; i < totaldrives; i++) {
    //      printf("%d ",stripecounter[i]);
    //    }
    //    printf("\n");
  }


  // histogram
  if (showhist) {
    //    printf("maximum stripid %d\n", stripid);
    int *striphist = calloc(stripid, sizeof(int));
    for (int r = 0 ; r < totalrows; r++) {
      for (int g = 0; g < groups; g++ ){
	for (int i = 0; i < drives; i++) {
	  int driveid = g * drives + i;
	  if (drivesok[driveid]) 
	    {
	      int v = a[r * totaldrives + driveid];
	      assert(v>=0);
	      //	      assert(v < stripid);
	      //	      printf("v = %d, max = %d\n", v, stripid);
	      if (v < stripid) {
		striphist[v]++;
	      }
	    }
	  
	}
      }
    }

    int *histhist = calloc(k+m+1, sizeof(int));
    assert(histhist);
    
    for (int i = 1; i < stripid; i++) {
      if (striphist[i]) {
	histhist[striphist[i]]++;
	if (striphist[i] < k+m) {
	  if (showhist >= 2) printf("strip %d = %d %s\n", i, striphist[i], striphist[i] < k ? "ERROR":"");
	  //	  if (striphist[i] < k)
	  //	    exit(1);
	  //
	}
      }
    }
    printf("\n");
    for (int i = 0; i <= k+m; i++) {
      printf("drives %d = %d\n", i, histhist[i]);
    }
    free(histhist);
  } else {
    
    for (int stripe = 0 ; stripe < Print; stripe++) {
      for (int g = 0; g < groups; g++ ){
	printf("| ");
	for (int i = 0; i < drives; i++) {
	  printf("%3d ", (int)a[stripe * totaldrives + (g*drives+i)]);
	}
      }
      printf("\n");
    }
  }

  free(shuf);
  free(stripecounter);
  free(a);
  return 0;
}
  
  

int main(int argc, char *argv[]) {

  srand48(time(NULL));

  
  const char *getoptstring = "hg:d:k:m:T:s:p:Hf:";

  int opt;
  int groups = 2;
  int drives = 13;
  int k = 6;
  int m = 3;
  int T = 1;
  int s = 1;
  int Print = 100;
  int showhist = 0;
  char *failed = NULL;
  
  while
    ((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 'h':
      usage();
      exit(1);
    case 'g': groups = atoi(optarg);break;
    case 'd': drives = atoi(optarg);break;
    case 'k': k = atoi(optarg);break;
    case 'm': m = atoi(optarg);break;
    case 'T': T = atoi(optarg);break;
    case 's': s = atoi(optarg);break;
    case 'p': Print = atoi(optarg); break;
    case 'H': showhist++; break;
    case 'f': failed = strdup(optarg); break;
    default:
      usage();
      exit(1);
    }
  }

  if (k+m > drives) {
    printf("*error* you can't have less drives than k+m\n");
    exit(1);
  }

  printf("*info* groups %d, drives/group %d, %d+%d, drive %d TB, stripe %d MB, failed '%s'\n", groups, drives, k, m, T, s, failed ? failed : "");
  
	
  int *drivesok = calloc(drives * groups, sizeof(int));
  for (int i = 0; i < drives * groups; i++) {
    drivesok[i] = 1;
  }
  if (failed) {
    if (atoi(failed) < 0) {
      // -2 means pick two random disks
      if (-atoi(failed) < drives * groups) { 
	for (int i = 0; i < -atoi(failed); i++) {
	  int d = 0;
	  while (drivesok[d=drand48()*(drives*groups)] == 0) {
	  }
	  drivesok[d] = 0;
	  printf("randomly failed drive %d\n", d);
	}
      }
    } else {
      char *first = strtok(failed, ",");
      while (first) {
	//      printf("%d\n", atoi(first));
	if (atoi(first) >= drives*groups) {
	  printf("Error, can't be >= %d\n", drives*groups);
	  exit(1);
	}
	drivesok[atoi(first)] = 0;
	first = strtok(NULL, ",");
      }
    }
  }

  int workingdrives = 0, faileddrives = 0;
  for (int i = 0; i< drives*groups; i++) {
    if (drivesok[i]) workingdrives++; else faileddrives++;
  }
  printf("*info* %d working drives, %d failed\n", workingdrives, faileddrives);
  
  simulate(drivesok, groups, drives, k, m, T, s, Print, showhist);
  

  return 0;
}
