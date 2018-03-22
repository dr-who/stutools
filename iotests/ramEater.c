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

  FILE *fp = fopen("/proc/sys/vm/overcommit_memory", "rt");
  if (!fp) {perror("overcommit_memory");exit(-1);}
  size_t value = 0;
  int f = fscanf(fp,"%zd", &value);
  if (f==1) {
    fprintf(stderr,"*info* overcommit_memory is %zd\n", value);
  }
  fclose(fp);
  
  fp = fopen("/proc/sys/vm/overcommit_ratio", "rt");
  if (!fp) {perror("overcommit_ratio");exit(-1);}
  value = 0;
  f = fscanf(fp,"%zd", &value);
  if (f==1) {
    fprintf(stderr,"*info* overcommit_ratio is %zd\n", value);
  }
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
    
    size_t size = 128*1024;
    while (1) {
      p[index] = realloc(p[index], size);
      memset(p[index], 'x', size);
      if (p[index]) {
	globalTotal += size;
	fprintf(stderr,"[Index %zd]: virtual allocation %.0lf GiB, total %0.lf GiB\n", index, TOGiB(size), TOGiB(globalTotal));
      } else {
	break;
      }
      
      size = size * 2;
    }
    index++;
    if (index >= maxArrays) {
      usleep(10);
      index--;
    }
  }
    
    
  

  return 0;
}

    



		  
