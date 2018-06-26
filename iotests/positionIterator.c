#include <stdio.h>
#include <stdlib.h>

#include <math.h>
#include <assert.h>

#include "positionIterator.h"
#include "utils.h"

int keepRunning = 0;

void positionIteratorInit(positionIteratorType *p, int alignment, size_t bdsize, size_t originalbdsize,
			  size_t low_bs, size_t high_bs) {
  p->type = EMPTYTYPE;
  const int alignbits = (int)(log(alignment)/log(2) + 0.01);
  p->alignbits = alignbits;
  p->low_bs = low_bs;
  p->high_bs = high_bs;
  assert (low_bs / alignment >= 1);
  assert (high_bs / alignment >= 1);
  p->BDSize = bdsize;
  p->originalBDSize = originalbdsize;
  p->maxAlignedBlocks = (bdsize / alignment); //[1M/1M blocks = 1 aligned block]
  fprintf(stderr,"maxBlocks %zd\n", p->maxAlignedBlocks);
  // if random then between [0 .. maxAlignedBlocks)... if K size, then make sure pos+K+len > bdsize
}

void positionRandom(positionIteratorType *p, int seed) {
  p->type = RANDOMTYPE;
  p->seed = seed;
  srand48(seed);
}

void positionLinear(positionIteratorType *p, int start) { // start in alignment blocks
  p->type = LINEARTYPE;
  p->lastpos = start << p->alignbits;
}

size_t positionNext(positionIteratorType *p, size_t *pos, size_t *len) {
  assert(p->type != EMPTYTYPE);
  const size_t thislen = randomBlockSize(p->low_bs, p->high_bs, p->alignbits);
  size_t b = 0;

  switch (p->type) {
  case RANDOMTYPE:
    do {
      b = (lrand48() % (p->maxAlignedBlocks+1)); // in blocks
      b = b << p->alignbits; // in bytes
    } while (b + thislen > p->BDSize);
    *pos = b;
    *len = thislen;
    break;
  case LINEARTYPE:
    *pos = p->lastpos;
    *len = thislen;

    b = *pos + thislen;
    if (b >= p->BDSize) {
      b = b - p->BDSize;
    }
    p->lastpos = b;
    break;
  }
      
  
  return 0;
}


int main() {
  positionIteratorType p;
  positionIteratorInit(&p, 1024, 1024*1024, 1024*1024, 4096, 65536);
  positionRandom(&p, 1);
  size_t pos, len;
  for (size_t i = 0; i < 2000000; i++) {
    positionNext(&p, &pos, &len);
    assert(pos + len <= 1024*1024);
    assert(len >= 4096);
    assert(len <= 65536);
    assert( (size_t)(len / 1024) * 1024 == len);
    
    //    fprintf(stdout, "%zd %zd\n", pos, len);
  }

  positionLinear(&p, 0);
  for (size_t i = 0; i < 2000000; i++) {
    positionNext(&p, &pos, &len);
    assert(pos + len <= 1024*1024);
    assert(len >= 4096);
    assert(len <= 65536);
    fprintf(stdout, "%zd %zd\n", pos, len);
  }

  return 1;
}



