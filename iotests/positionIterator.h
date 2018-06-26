#ifndef _POSITIONITERATOR_H
#define _POSITIONITERATOR_H

#define EMPTYTYPE 0
#define RANDOMTYPE 1
#define LINEARTYPE 2
#define HOPTYPE 3

typedef struct {
  int type;
  int alignbits;
  int low_bs;
  int high_bs;
  size_t BDSize;
  size_t originalBDSize;
  size_t maxAlignedBlocks;
  size_t lastpos;
  int seed;
  int jumplow;
  int jumphigh;
} positionIteratorType;


#endif
