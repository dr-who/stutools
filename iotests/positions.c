#define _GNU_SOURCE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>

#include "utils.h"
#include "positions.h"

extern int verbose;

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
	  con = alignedNumber(lrand48(), bs); // % (bdSizeBytes / bs)) * bs;
	  if (startAtZero) con = 0;
	}
      }
      positions[i].pos = con;
      positions[i].len = randomBlockSize(lowbs, bs, alignment);
    }
  } else {
    // parallel contiguous regions
    size_t *ppp = NULL, totallen = 0;
    int abssf = ABS(sf);
    if (abssf <= 0) abssf = 1;
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
    } //finish setup
    
      // now iterate 
    for (size_t i = 0; i < num; i++) {
      // sequential
      size_t rbs = randomBlockSize(lowbs, bs, alignment);
      positions[i].pos = ppp[i % abssf];// / bs)) * bs;
      positions[i].len = rbs;
      
      ppp[i % abssf] += rbs;
      totallen += rbs;
      if (ppp[i % abssf] + bs > bdSizeBytes) { // if could go off the end then set back to 0
	ppp[i % abssf] = 0;
      }
    }
    if (totallen > bdSizeBytes) {
      fprintf(stderr,"*warning* sum position size is too large! will have overlapping access (%zd positions)\n", num);
      fprintf(stderr,"*warning* sum of position sizes %zd (%.2lf GiB) > %zd (%.2lf GiB)\n", totallen, TOGiB(totallen), bdSizeBytes, TOGiB(bdSizeBytes));
    }
    free(ppp);
    
    if (sf < 0) {
      // reverse positions array
      if (verbose) {
	fprintf(stderr,"*info* reversing positions\n");
      }
      for (size_t i = 0; i < num/2; i++) {

	positionType p = positions[i];
	positions[i] = positions[num-1 -i];
	positions[num-1 -i] = p;
      }
    }
  }

  // if randomise then reorder
  if (sf == 0) {
    for (size_t shuffle = 0; shuffle < 3; shuffle++) {
      if (verbose) {
	fprintf(stderr,"*info* shuffling the array (%zd). Shuffle %zd\n", num, shuffle);
      }
      for (size_t i = 0; i < num; i++) {
	size_t j = i;
	if (num > 1) {
	  while ((j = lrand48() % num) == i) {
	    ;
	  }
	}
	// swap i and j
	positionType p = positions[i];
	positions[i] = positions[j];
	positions[j] = p;
	if (verbose > 1 && i < 10) {
	  fprintf(stderr,"*info* [%zd]: %zd/%zd (was %zd/%zd)\n", i, positions[i].pos, positions[i].len, positions[j].pos, positions[j].len);
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
    p->success = 0;
    assert(p->len); // can't be zero
    assert(p->len >= lowbs);
    assert(p->len <= bs);
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
