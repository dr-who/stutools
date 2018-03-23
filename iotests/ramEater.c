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

  FILE *fp = fopen("/proc/sys/vm/overcommit_ratio", "rt");
  if (!fp) {perror("overcommit_ratio");exit(-1);}
  size_t value = 0;
  int f = fscanf(fp,"%zd", &value);
  if (f==1) {
    fprintf(stderr,"*info* overcommit_ratio is %zd\n", value);
  }
  fclose(fp);
  if (value != 100) {fprintf(stderr,"*error* the value needs to be 100\n  echo 100 > /proc/sys/vm/overcommit_ratio\n");exit(-1);}

  fp = fopen("/proc/sys/vm/overcommit_memory", "rt");
  if (!fp) {perror("overcommit_memory");exit(-1);}
  value = 0;
  f = fscanf(fp,"%zd", &value);
  if (f==1) {
    fprintf(stderr,"*info* overcommit_memory is %zd\n", value);
  }
  fclose(fp);
  if (value != 2) {fprintf(stderr,"*error* the value needs to be 2. \n  echo 2 > /proc/sys/vm/overcommit_memory\n");exit(-1);}
  

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
	memset(p[index] + oldsize, 'x', size-oldsize);
	globalTotal += (size - oldsize);
      } else {
	break;
      }

      oldsize = size;
      size = size * 5/4;
      //      if (size > 1024*1024*1024) break;
    }
    index++;
    if (index >= maxArrays) {
      //      usleep(10);
      index--;
    }
  }
    
    
  

  return 0;
}

    



		  
