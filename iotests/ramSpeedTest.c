#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "utils.h"

int keepRunning = 1;

char *ramTest_alloc(size_t numBlocks, size_t blockSize) {
  //  fprintf(stderr,"*info* allocating %.1lf GB\n", TOGiB(numBlocks * blockSize));
  char *data;
  CALLOC(data, numBlocks * blockSize, 1);

  return data;
}


void ramTest_test(char *ram, size_t numBlocks, size_t blockSize, float secs) {
  double start = timedouble(), elapsed = start, thistime = start;
  srand48(1);

#define POSITIONS (10*1000*1000)
  
  // create random positions
  size_t *positions = malloc(POSITIONS * sizeof(size_t));
  for (size_t i = 0; i < POSITIONS; i++) {
    positions[i] = (lrand48() % numBlocks) * blockSize;
  }

  char *from = ramTest_alloc(1, blockSize);

  size_t count = 0;
  size_t *p = positions;
  while ((thistime = timedouble()) - start < secs) {
    for (size_t i = 0; i < 10000; i++) { // group number of writes
      memcpy(ram + *p, from, blockSize);
      p++;
      count++;
      if (p - positions >= POSITIONS) {
	p = positions;
      }
    }

    //    fprintf(stderr,"%.2f\n", thistime - last);
    //    if (thistime - last >=1) {
      //      fprintf(stderr," ....%.2lf GB/s", TOGiB(count * blockSize) / elapsed);
      // every second
    //      last = thistime;
    //    }
  }
  free(positions);
  free(from);
  
  elapsed = thistime - start;
  fprintf(stderr,"blocksize %6zd, time %5.2lf, count %8zd, %5.2lf GB/s\n", blockSize, elapsed, count, TOGiB(count * blockSize) / elapsed);
}


int main(int argc, char *argv[]) {
  size_t bytes = 512UL*1024*1024;
  size_t sec = 7;

#define TESTS 5
  size_t b[TESTS] = {1024, 4096, 16384, 65536, 65536*4};
  for (size_t i = 0; i < TESTS; i++) {
    size_t blocks = bytes / b[i];
    
    char *ram = ramTest_alloc(blocks, b[i]);
    ramTest_test(ram, blocks, b[i], sec);
    free(ram);
  }

  return 0;
}

    



		  
