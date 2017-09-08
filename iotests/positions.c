#define _GNU_SOURCE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>

#include "utils.h"
#include "positions.h"

// sorting function, used by qsort
static int poscompare(const void *p1, const void *p2)
{
  const positionType *pos1 = (positionType*)p1;
  const positionType *pos2 = (positionType*)p2;
  if (pos1->pos < pos2->pos) return -1;
  else if (pos1->pos > pos2->pos) return 1;
  else return 0;
}



positionType *createPositions(size_t num) {
  positionType *p;
  CALLOC(p, num, sizeof(positionType));
  return p;
}


void checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes) {
  //  fprintf(stderr,"... checking\n");
  size_t rcount = 0, wcount = 0;
  size_t sizelow = -1, sizehigh = 0;
  positionType *p = (positionType *)positions;
  for (size_t j = 0; j < num; j++) {
    if (p->len < sizelow) {
      sizelow = p->len;
    }
    if (p->len > sizehigh) {
      sizehigh = p->len;
    }
    if (p->action == 'R') {
      rcount++;
    } else {
      wcount++;
    }
    p++;
  }

  // check all positions are aligned to low and high lengths
  p = (positionType *) positions;
  if (sizelow > 0) {
    for (size_t j = 0; j < num; j++) {
      if (p->pos + sizehigh > bdSizeBytes) {
	fprintf(stderr,"eek off the end of the array %zd!\n", p->pos);
      }
      if ((p->pos % sizelow) != 0) {
	fprintf(stderr,"eek no %zd aligned position at %zd!\n", sizelow, p->pos);
      }
      if ((p->pos % sizehigh) != 0) {
	fprintf(stderr,"eek no %zd aligned position at %zd!\n", sizehigh, p->pos);
      }
      p++;
    }
  }

  // duplicate, sort the array. Count unique positions
  positionType *copy;
  CALLOC(copy, num, sizeof(positionType));
  memcpy(copy, positions, num * sizeof(positionType));
  qsort(copy, num, sizeof(positionType), poscompare);
  // check order
  size_t unique = 0;
  for (size_t i = 1; i <num; i++) {
    if (copy[i].pos != copy[i-1].pos) {
      unique++;
      if (i==1) { // if the first number checked is different then really it's 2 unique values.
	unique++;
      }
    }
    if (copy[i].pos < copy[i-1].pos) {
      fprintf(stderr,"not sorted %zd %zd, unique %zd\n",i, copy[i].pos, unique);
    }
  }
  free(copy);

  fprintf(stderr,"action summary: reads %zd, writes %zd, checked ratio %.1lf, len [%zd, %zd], unique positions %zd\n", rcount, wcount, rcount*1.0/(rcount+wcount), sizelow, sizehigh, unique);
}


// lots of checks
void dumpPositions(positionType *positions, size_t num, size_t bdSizeBytes) {
  //  fprintf(stderr,"... dump positions\n");
  for (size_t i = 0; i <num; i++) {
    fprintf(stdout,"%10zd\t%.2lf GiB\t%c\t%zd\t%zd\t%.2lf GiB\t(%.1lf%%)\n", positions[i].pos, TOGiB(positions[i].pos), positions[i].action, positions[i].len, bdSizeBytes, TOGiB(bdSizeBytes), positions[i].pos * 100.0 / bdSizeBytes);
  }
}


void createLinearPositions(positionType *positions,
			   const size_t numPositions,
			   const size_t blockSize,
			   const size_t bdSizeBytes,     // max block device size in bytes
			   const size_t startBlock, // start block * bdSize
			   const size_t finBlock,   // finis block * bdSize
			   const int    jumpBlock,
			   const double rwRatio) {

  if (bdSizeBytes < blockSize) {
    fprintf(stderr,"*warning* size of device is less than block size!\n");
    return;
  }
  size_t restarts = 0;
  assert(jumpBlock > 0);
  assert(finBlock > startBlock);
  assert(rwRatio >= 0 && rwRatio <= 1);
  positionType *p = positions;

  size_t blockIndex = startBlock;
  for (size_t i = 0; i < numPositions; i++) {
    if ((blockIndex > finBlock) ||( (blockIndex+1) * blockSize > bdSizeBytes)) {
      blockIndex = startBlock;
      restarts++;
    }
    p->pos = blockIndex * blockSize; // position is index * blockSize
    if (drand48() < rwRatio) {
      p->action = 'R';
    } else {
      p->action = 'W';
    }
    p->len = blockSize;
    p->success = 0;
    
    blockIndex = blockIndex + jumpBlock;
    p++;
  }
}


/*void createParallelPositions(positionType *positions,
			     const size_t numPositions,
			     const size_t blockSize,
			     const size_t bdSizeBytes,
			     const int    jumpBlock,
			     const double rwRatio,
			     const size_t parallelRegions) {

  if (bdSizeBytes < blockSize) {
    fprintf(stderr,"*warning* size of device is less than block size!\n");
    return;
  }

  assert(numPositions >= 1);
  assert(blockSize % 1024 == 0);
  assert(jumpBlock >= 1);
  assert(rwRatio >= 0.0 && rwRatio <= 1.0);
  assert(parallelRegions > 0 && parallelRegions < 1000000);
  
  size_t *ppp = NULL;
  CALLOC(ppp, parallelRegions, sizeof(size_t));

  for (size_t i = 0; i < parallelRegions; i++) {
    size_t startSeqPos = 0;
    size_t count = 0;
    while (1) {
      startSeqPos = alignedNumber(lrand48() % bdSizeBytes, alignment);// % (bdSizeBytes / blockSize)) * blockSize;
      if (i == 0) {
	break;
      }
      
      int close = 0;
      for (size_t j = 0; j < i-1; j++) {
	if (ABS(ppp[j] - startSeqPos) < 100*1024*1024) { // make sure all the points are 100MB apart
	  close = 1;
	}
      }
      if (!close) {
	break;
      }
      count++;
      if (count > 100) {
	fprintf(stderr,"*error* can't make enough spaced positions given the size of the disk\n");
	exit(1);
      }
      //	    fprintf(stderr,"count++\n");
    }
    ppp[i] = startSeqPos;
  }
  

  size_t pppIndex = 0;
  for (size_t i = 0; i < numPositions; i++) {
    if (ppp[pppIndex] + blockSize > bdSizeBytes) {
      ppp[pppIndex] = 0;
    }
    positions[i].pos = ppp[pppIndex];
    if (drand48() < rwRatio) {
      positions[i].action = 'R';
    } else {
      positions[i].action = 'W';
    }
    positions[i].len = blockSize;
    positions[i].success = 0;

    ppp[pppIndex] += (jumpBlock * blockSize);
    pppIndex++;
    if (pppIndex >= parallelRegions) {
      pppIndex = 0;
    }
  }
}


*/

// create the position array
void setupPositions(positionType *positions,
			  const size_t num,
			  const size_t bdSizeBytes,
			  const int sf,
			  const double readorwrite,
			  const size_t lowbs,
			  const size_t bs,
		          const size_t alignment,
			  const size_t singlePosition,
		          const int    jumpStep,
		          const size_t startAtZero
		    ) {
  assert (lowbs <= bs);
  if (bdSizeBytes < bs) {
    fprintf(stderr, "*warning* size of device is less than block size!\n");
    return;
  }
  
  if (singlePosition) {
    size_t con = (lrand48() % (bdSizeBytes / bs)) * bs;
    if (startAtZero) con = 0;
    fprintf(stderr, "Using a single block position: %zd (singlePosition value %zd)\n", con, singlePosition);
    for (size_t i = 0; i < num; i++) {
      if (singlePosition > 1) {
	if ((i % singlePosition) == 0) {
	  con = alignedNumber(lrand48(), alignment); // % (bdSizeBytes / bs)) * bs;
	  if (startAtZero) con = 0;
	}
      }
      positions[i].pos = con;
    }
  } else {
    // random positions
    if (sf == 0) {
      for (size_t i = 0; i < num; i++) {
	if (startAtZero && (i==0)) {
	  positions[i].pos = 0;
	} else {
	  positions[i].pos = alignedNumber(lrand48() % bdSizeBytes, alignment);// % (bdSizeBytes / bs)) * bs;
	}
      }
    } else {
      // parallel contiguous regions
      size_t *ppp = NULL;
      int abssf = ABS(sf);
      CALLOC(ppp, abssf, sizeof(size_t));
      for (size_t i = 0; i < abssf; i++) {
	size_t startSeqPos = 0;
	size_t count = 0;
	while (1) {
	  startSeqPos = (lrand48() % (bdSizeBytes / bs)) * bs;
	  if ((i == 0) && startAtZero) {
	    startSeqPos = 0;
	  }
	  if (i == 0) {
	    break;
	  }
	  
	  int close = 0;
	  for (size_t j = 0; j < i-1; j++) {
	    if (ABS(ppp[j] - startSeqPos) < 100*1024*1024) { // make sure all the points are 100MB apart
	      close = 1;
	    }
	  }
	  if (!close) {
	    break;
	  }
	  count++;
	  if (count > 100) {
	    fprintf(stderr,"*error* can't make enough spaced positions given the size of the disk\n");
	    exit(1);
	  }
	  //	    fprintf(stderr,"count++\n");
	}
	ppp[i] = startSeqPos;
	//	  fprintf(stderr,"i=%zd val %zd\n", i, startSeqPos);
      }
      
      for (size_t i = 0; i < num; i++) {
	// sequential
	positions[i].pos = alignedNumber(ppp[i % abssf], alignment);// / bs)) * bs;
	ppp[i % abssf] += (jumpStep * bs);
	if (ppp[i % abssf] + bs > bdSizeBytes) { // if could go off the end then set back to 0
	  ppp[i % abssf] = 0;
	}
      }
      free(ppp);

      if (sf < 0) {
	// reverse positions array
	fprintf(stderr,"*info* reversing positions\n");
	for (size_t i = 0; i < num/2; i++) { 
	  size_t temp = positions[i].pos;
	  positions[i].pos = positions[num-1 - i].pos;
	  positions[num-1 -i].pos = temp;
	}
      }
    }
  }

  // setup R/W
  positionType *p = positions;
  for (size_t j = 0; j < num; j++) {
    if (drand48() <= readorwrite) {
      p->action = 'R';
    } else {
      p->action = 'W';
    }

    size_t lowbs_k = lowbs / alignment; // 1 / 4096 = 0
    if (lowbs_k < 1) lowbs_k = 1;
    size_t highbs_k = bs / alignment;   // 8 / 4096 = 2
    if (highbs_k < 1) highbs_k = 1;

    size_t randombs_k = lowbs_k;
    if (highbs_k > lowbs_k) {
      randombs_k += (lrand48() % (highbs_k - lowbs_k + 1));
    }
    
    size_t randombs = alignedNumber(randombs_k * alignment, alignment);
    if (randombs <= 0) {
      randombs = alignment;
    }
    p->len = randombs;
    p->success = 0;
    p++;
  }
}


/*int main() {
  srand48((long)timedouble());
  const size_t num = 10*1024*1024;
  positionType *p = createPositions(num);

  size_t bdSizeBytes = 2uL*1024uL*1024uL*1024uL -1;
  createLinearPositions(p, num, 65536, bdSizeBytes, 0, 10000000, 1.0, 1);
  createParallelPositions(p, num, 65536, bdSizeBytes, 10, 1.0, 10);

  //  checkPositionArray(p, num, bdSizeBytes);
  dumpPositions(p, num, bdSizeBytes);
  }*/
