#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "utils.h"

int keepRunning = 1;


int main(int argc, char *argv[]) {
  size_t swap = swapTotal();
  if (swap) {
    fprintf(stderr,"*error* swap needs to be turned off (%.1lf GiB)\n", TOGiB(swap));
    exit(-1);
  }

  if (swapTotal() > 0) {
    fprintf(stderr,"*error* ramEater needs swap to be off. `sudo swapoff -a`\n");
    exit(-1);
  }

  FILE *fp = fopen("/proc/self/oom_score_adj", "wt");
  fprintf(fp,"1000\n");
  fclose(fp);

  size_t maxArrays = 1024*1024;
  void **p;
  CALLOC(p, maxArrays, sizeof(void*));
  
  for (size_t i = 0; i < maxArrays; i++) {
    p[i] = NULL;
  }

  size_t globalTotal = 0;
  size_t index = 0;
  while (1) {
    
    size_t size = 1024, oldsize = size;
    while (1) {
      fprintf(stderr,"[Index %zd]: virtual allocation with size %zd, total %0.lf GiB\n", index, size, TOGiB(globalTotal));
      
      p[index] = realloc(p[index], size);
      if (p[index]) {
	memset((char*)p[index] + oldsize, 'x', size-oldsize);
	globalTotal += (size - oldsize);
      } else {
	fprintf(stderr,"sleep\n");
	sleep(10);
	continue;
	//	break;
      }

      oldsize = size;
      size = size + (65536*10);
      if (size > 1024*1024*1024) break;
    }
    index++;
    if (index >= maxArrays) {
      //      usleep(10);
      index--;
    }
  }
    
    
  

  return 0;
}

    



		  
