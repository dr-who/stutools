#include <stdlib.h>
#include <stdio.h>

#include "procDiskStats.h"

int main(int argc, char *argv[]) {

  int opt;
  const char *getoptstring = "l:t:";

  float timems = 1000;
  float latencyms = 0;
  
  while((opt = getopt(argc, argv, getoptstring)) != -1) {
    switch (opt) {
    case 't':
      timems = atof(optarg) * 1000.0;
      break;
    case 'l':
      latencyms = atof(optarg);
      break;
    }
  }

  fprintf(stderr,"*info* time %.0lf ms, latency %.0lf ms\n", timems, latencyms);

  procDiskStatsType old,new, delta;

  procDiskStatsInit(&old);
  procDiskStatsSample(&old);
  while (1) {

    usleep(timems * 1000.0);
    
    procDiskStatsInit(&new);
    procDiskStatsSample(&new);
    
    delta = procDiskStatsDelta(&old, &new);
    
    procDiskStatsDumpThres(&delta, latencyms);
    procDiskStatsFree(&delta);

    procDiskStatsCopy(&old, &new);

    procDiskStatsFree(&new);
  }

  procDiskStatsFree(&new);
  
  exit(0);
}

  
