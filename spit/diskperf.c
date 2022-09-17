#include <stdlib.h>
#include <stdio.h>

#include "procDiskStats.h"
#include "utils.h"

int keepRunning = 1;

void usage(float timems, float latencyms, float zscore) {
  fprintf(stderr,"Usage:\n  diskperf [-s seconds] [options...] [-l latencyms] [-z 5 (default)]\n");
  fprintf(stderr,"\nOptions:\n");
  fprintf(stderr,"  -s n  \tsample time of n seconds (default %.0lf)\n", timems/1000.0);
  fprintf(stderr,"  -d n  \tstop after n iterations of display\n");
  fprintf(stderr,"  -l n  \tprint IO for drives that over n ms of avg latency (default %.0lf)\n", latencyms);
  fprintf(stderr,"  -z n  \tprint IO for drives that have a total IO in the sample (>= z-score= %.1lf)\n", zscore);
}

int main(int argc, char *argv[]) {

  int opt;
  const char *getoptstring = "l:s:hd:z:";

  float timems = 1000;
  float latencyms = 0;
  float zscore = 5;
  size_t stopaftern = (size_t)-1;
  
  while((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 's':
      timems = atof(optarg) * 1000.0;
      break;
    case 'z':
      zscore = atof(optarg);
      latencyms = 0;
      break;
    case 'l':
      latencyms = atof(optarg);
      zscore = 0;
      break;
    case 'd':
      stopaftern = atoi(optarg);
      break;
    case 'h':
      usage(timems, latencyms, zscore);
      exit(1);
    }
  }

  // check only one running
  int canopen = openRunLock("/var/run/stutools-diskperf");
  if (!canopen) {
    fprintf(stderr,"*error* diskperf already running... exiting...\n");
    exit(0);
  }

  // now run
  double starttime = timedouble();

  mapVoidType *map_r = NULL, *map_w = NULL;
  if (zscore > 0) {
    map_r = calloc(1, sizeof(mapVoidType));
    map_w = calloc(1, sizeof(mapVoidType));
    mapVoidInit(map_r);
    mapVoidInit(map_w);
    fprintf(stderr,"*info* time %.0lf ms, alerts based on z-score >= %.1lf\n", timems, zscore);
  } else {
    // use latency values
    fprintf(stderr,"*info* time %.0lf ms, print if latency >= %.0lf ms\n", timems, latencyms);
  }
  
  

  procDiskStatsType old,new, delta;

  procDiskStatsInit(&old);
  procDiskStatsSample(&old);
  size_t displaycount = 0;

  while (++displaycount <= stopaftern) {

    usleep(timems * 1000.0);
    
    procDiskStatsInit(&new);
    procDiskStatsSample(&new);
    
    delta = procDiskStatsDelta(&old, &new);
    delta.startTime = starttime;

    procDiskStatsDumpThres(stdout, &delta, latencyms, map_r, map_w, zscore);fflush(stdout);
    procDiskStatsFree(&delta);

    procDiskStatsFree(&old);
    procDiskStatsCopy(&old, &new);

    procDiskStatsFree(&new);
  }

  procDiskStatsFree(&old);
  
  exit(0);
}

  
