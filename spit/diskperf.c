#include <stdlib.h>
#include <stdio.h>

#include "procDiskStats.h"

int main() {

  procDiskStatsType old,new, delta;

  procDiskStatsInit(&old);
  procDiskStatsSample(&old);
  while (1) {

    sleep(2);
    
    procDiskStatsInit(&new);
    procDiskStatsSample(&new);
    
    delta = procDiskStatsDelta(&old, &new);
    
    procDiskStatsDump(&delta);
    procDiskStatsFree(&delta);

    procDiskStatsCopy(&old, &new);

    procDiskStatsFree(&new);
  }

  procDiskStatsFree(&new);
  
  exit(0);
}

  
