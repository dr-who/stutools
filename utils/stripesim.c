#define _XOPEN_SOURCE 500

#include <stdio.h>
#include <time.h>
#include <stdlib.h>
#include <unistd.h>
#include <getopt.h>
#include <assert.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>



void usage() {
  printf("usage: stripsim -g 2 -d 13 -k 6 -m 3 -T 20 -s 1\n");
  printf("\n");
  printf("options:\n");
  printf("   -g n      groups of drives\n");
  printf("   -d n      drives per group (total = g x d)\n");
  printf("   -t n      specify number of drives for group calc (d=t/g)\n");
  printf("   -k/m      data/parity\n");
  printf("   -T        size of drive in TB\n");
  printf("   -S n      n samples\n");
  printf("   -s        stripe size in MB\n");
  printf("   -H        print histogram\n");
  printf("   -H -H     verbose histogram\n");
  printf("   -f 0,2... list of failed drives\n");
  printf("   -f -5     if negative, fail that many drives\n");
  printf("   -p n      print n strips\n");
}


int simulate(unsigned int *a, int *drivesok, int groups, int drives, int k, int m, int T, int s, int Print, int showhist) {
  size_t totaldrives = groups * drives;
  size_t totalrows = T*1000L*1000L / s;

  assert(a);

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
	int i = drand48() * drives;
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
	  fprintf(stderr, "*warning* breaking out!\n");
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

  int dataloss = 0;


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
	      if ((v > 0) && (v < stripid)) {
		striphist[v]++;
		//		if (striphist[v] >= totaldrives) abort();
	      }
	    }
	  
	}
      }
    }

    int *histhist = calloc(totaldrives+1, sizeof(int));
    assert(histhist);

    for (int i = 1; i < stripid; i++) {
      if (striphist[i]) {
	if ((striphist[i] >= 0) && (striphist[i] <=totaldrives)) {
	  histhist[striphist[i]]++;
	  if (striphist[i] < k+m) {
	    if (showhist >= 3) printf("strip %d = %d %s\n", i, striphist[i], striphist[i] < k ? "ERROR":"");
	    //	  if (striphist[i] < k)
	    //	    exit(1);
	    //
	  }
	} else {
	  //	  printf("warning: %d! total drives %zd \n", striphist[i], totaldrives);
	}
      }
    }
    free(striphist); striphist = NULL;
    if (showhist==1) {
      for (int i = 1; i <= k+m; i++) {
	printf("drives %d = %d\n", i, histhist[i]);
	if ((i < k) && (histhist[i])) {
	  dataloss = 1;
	}
      }
    } else {
      for (int i = 1; i <= k+m; i++) {
	printf("%d\t", histhist[i]);
	if ((i < k) && (histhist[i])) {
	  dataloss = 1;
	}
      }
      printf("\n");
    }
    free(histhist); histhist = NULL;
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
  return dataloss;
}


int *failedDrives(int totaldrives, char *failed) {
  
  int *drivesok = calloc(totaldrives, sizeof(int));
  for (int i = 0; i < totaldrives; i++) {
    drivesok[i] = 1;
  }
  if (failed) {
    if (atoi(failed) < 0) {
      // -2 means pick two random disks
      if (-atoi(failed) < totaldrives) { 
	for (int i = 0; i < -atoi(failed); i++) {
	  int d = 0;
	  while (drivesok[d=drand48()*totaldrives] == 0) {
	  }
	  drivesok[d] = 0;
	  //fprintf(stderr,"*info* randomly failed drive %d\n", d);
	}
      }
    } else {
      char *first = strtok(failed, ",");
      while (first) {
	//      printf("%d\n", atoi(first));
	if (atoi(first) >= totaldrives) {
	  fprintf(stderr,"*error*, can't be >= %d\n", totaldrives);
	  exit(1);
	}
	drivesok[atoi(first)] = 0;
	first = strtok(NULL, ",");
      }
    }
  }

  int workingdrives = 0, faileddrives = 0;
  for (int i = 0; i< totaldrives; i++) {
    if (drivesok[i]) workingdrives++; else faileddrives++;
  }
  //  fprintf(stderr, "*info* %d working drives, %d failed\n", workingdrives, faileddrives);

  return drivesok;
}



int main(int argc, char *argv[]) {

  unsigned int seed = 42;
  int fd = open("/dev/random", O_RDONLY);
  if (fd) {
    int len = read(fd, &seed, sizeof(unsigned int));
    if (len != sizeof(unsigned int)) {
      printf("*warning* short /dev/random read\n");
    }
    close(fd);
  }

  fprintf(stderr, "*info* seed %u\n", seed);
  srand48(seed);

  
  const char *getoptstring = "hg:d:k:m:T:s:p:Hf:t:S:";

  int opt;
  int groups = 2;
  int drives = 13;
  int k = 6;
  int m = 3;
  int T = 1;
  int totaldrives = 0;
  int s = 1;
  int samples = 1;
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
    case 't': totaldrives = atoi(optarg);break;
    case 's': s = atoi(optarg);break;
    case 'S': samples = atoi(optarg); break;
    case 'p': Print = atoi(optarg); break;
    case 'H': showhist++; break;
    case 'f': failed = strdup(optarg); break;
    default:
      usage();
      exit(1);
    }
  }

  // if -t specified
  if (totaldrives > 0) {
    drives = totaldrives / groups;
  } else {
    totaldrives = drives * groups;
  }

  if (k+m > drives) {
    printf("*error* you can't have less drives than k+m\n");
    exit(1);
  }

  fprintf(stderr, "*info* groups %d, drives/group %d, %d+%d, drive %d TB, stripe %d MB, failed '%s' (samples=%d)\n", groups, drives, k, m, T, s, failed ? failed : "", samples);
  
	
  if (samples > 1) {
    showhist = 2;
  }

  size_t totalrows = T*1000L*1000L / s;

  //  fprintf(stderr,"*info* totalrows %zd, total drives %d\n", totalrows, totaldrives);
  size_t allocsize = totaldrives * (totalrows +1);
  fprintf(stderr, "*info* allocating = %.3lf GiB (%d x %zd = %zd bytes)\n", allocsize / 1024.0/1024.0/1024.0, totaldrives, totalrows+1, allocsize);
    
  unsigned int *a = calloc(allocsize , sizeof(unsigned int));
  if (!a) {printf("OOM!, trying to allocate %zd \n", allocsize * sizeof(int));exit(1); }

  int dataloss = 0;
  int ok = 0, bad = 0;
  
  
  for (int run = 0; run < samples; run++) {
    int *drivesok = failedDrives(groups * drives, failed);
    dataloss = simulate(a, drivesok, groups, drives, k, m, T, s, Print, showhist);
    if (dataloss) {
      bad++;
      fprintf(stderr,"*warning* stripe loss\n");
    } else {
      ok++;
    }
    free(drivesok);
  }
  fprintf(stderr,"*info* summary OK = %.1f%%, failed = %.1f%%\n", ok *100.0/samples, bad*100.0/samples);
  

  if (failed) {free(failed); failed=NULL;}

  free(a);

  return 0;
}
