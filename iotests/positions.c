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
		    size_t *num,
		          const int *fdArray,
		          const size_t fdSize,
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

  // list of possibles positions
  positionType *poss;
  size_t possAlloc = 100, startPos = 0, count = 0;
  CALLOC(poss, possAlloc, sizeof(positionType));
  while (startPos + bs < bdSizeBytes) {
    poss[count].pos = startPos;
    poss[count].len = randomBlockSize(lowbs, bs, alignment);
    
    startPos += poss[count].len;
    count++;
    if (count >= possAlloc) {
      possAlloc = possAlloc * 2;
      poss = realloc(poss, possAlloc * sizeof(positionType));
      //      fprintf(stderr,"realloc\n");
    }
  } // find a position across the disks
  if (verbose > 1) {
    fprintf(stderr,"*info* %zd unique positions, allocated %zd positions\n", count, *num);
  }
  if (*num > count) {
    fprintf(stderr,"*warning* there are %zd unique positions on the device\n", count);
    *num = count;
  }

  if (startAtZero) {
    poss[0].pos = 0;
  }

  int fdPos = 0;
  if (singlePosition) {
    // set a few fixed positions
    fprintf(stderr, "Using a single block position: singlePosition value of %zd\n", singlePosition);
    
    size_t sinPos = lrand48() % count;
    for (size_t i = 0; i < *num; i++) {
      if ((singlePosition > 1) && ((i % singlePosition) == 0)) {
	sinPos = lrand48() % count;
      }

      positions[i].pos = poss[sinPos].pos;
      positions[i].len = poss[sinPos].len;
      positions[i].fd = fdArray[fdPos++];
      if (fdPos >= fdSize) fdPos = 0;
    }
  } else {
    
    // if randomise then reorder
    if (sf == 0) {
      for (size_t shuffle = 0; shuffle < 1; shuffle++) {
	if (verbose > 1) {
	  fprintf(stderr,"*info* shuffling the array %zd\n", count);
	}
	for (size_t i = 0; i < count; i++) {
	  size_t j = i;
	  if (count > 1) {
	    while ((j = lrand48() % count) == i) {
	      ;
	    }
	  }
	  // swap i and j
	  positionType p = poss[i];
	  poss[i] = poss[j];
	  poss[j] = p;
	  if (verbose > 1 && i < 10) {
	    fprintf(stderr,"*info* [%zd]: %zd/%zd (was %zd/%zd)\n", i, poss[i].pos, poss[i].len, poss[j].pos, poss[j].len);
	  }
	}
      }
    }

    
    if (verbose) {
      fprintf(stderr,"*info* generating parallel contiguous regions, num %zd, #fd = %zd, abssf=%d\n", *num, fdSize, sf);
    }

    // setup the starting positions for parallel contiguous regions
    size_t *ppp = NULL, *ppp2 = NULL, totallen = 0;
    int abssf = ABS(sf);
    if (abssf <= 0) abssf = 1;
    *num = abssf * (size_t)((*num) /abssf);

    size_t offset = lrand48() % count; // offset in the list of possible positions
    
    CALLOC(ppp, abssf, sizeof(size_t)); // start in 'abssf' regions
    CALLOC(ppp2, abssf, sizeof(size_t)); // start in 'abssf' regions
    // if 1 start at 0 + offset
    // if 2 start at 0 + offset and n/2 + offset
    // if 3 start at 0 + offset and n/3 + offset and 2n/3 + offset
    for (size_t i = 0; i < abssf; i++) {
      ppp[i] = i * count / abssf + offset;  //start
      ppp2[i] = (i+1) * count / abssf + offset; //end
      if (ppp2[i] > count) ppp2[i] -= count;
      if (ppp[i] > count) ppp[i] -= count;
      if (verbose > 1) {
	size_t lb = 0;
	for (size_t j = 0; j < count / abssf; j++) {
	  size_t idx = ppp[i] + j;
	  if (idx > count) idx -= count;
	  lb += poss[idx].len;
	}
	fprintf(stderr,"*info* sequence %zd, position array index [%zd - %zd] (%.1lf GiB)\n", i + 1, ppp[i], ppp2[i],TOGiB(lb));
      }
    } //finish setup
    
      // now iterate
    fdPos = 0;
    size_t i = 0;
    while (i < *num) {
    //    for (size_t i = 0; i < *num; i++) {
      // sequential
      if (ppp[i % abssf] == ppp2[i % abssf] && (i)) {
	fprintf(stderr,"*error* overlapping regions\n");
      }

      positions[i].pos = poss[ppp[i % abssf]].pos; // dereference
      positions[i].len = poss[ppp[i % abssf]].len;
      positions[i].fd = fdArray[fdPos++];
      if (fdPos >= fdSize) fdPos = 0;
      
      ppp[i % abssf]++;

      if (ppp[i % abssf] >= count) {
	ppp[i % abssf] = 0; // back to the start
      }
      totallen += positions[i].len;
      //fprintf(stderr,"** %d %zd %zd\n", positions[i].fd, positions[i].pos, positions[i].len);
      i++;
    }
    if (totallen > bdSizeBytes) {
      fprintf(stderr,"*warning* sum position size is too large! will have overlapping access (%zd positions)\n", *num);
      fprintf(stderr,"*warning* sum of position sizes %zd (%.2lf GiB) > %zd (%.2lf GiB)\n", totallen, TOGiB(totallen), bdSizeBytes, TOGiB(bdSizeBytes));
    }
    free(ppp);
    free(ppp2);
    
    if (sf < 0) {
      // reverse positions array
      if (verbose) {
	fprintf(stderr,"*info* reversing positions\n");
      }
      for (size_t i = 0; i < (*num)/2; i++) {

	positionType p = positions[i];
	positions[i] = positions[(*num)-1 -i];
	positions[(*num)-1 -i] = p;
      }
    }
  }

  // setup R/W
  positionType *p = positions;
  for (size_t j = 0; j < *num; j++) {
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

  free(poss); // free the possible locations
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
