#define _GNU_SOURCE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <math.h>

#include "utils.h"
#include "positions.h"
#include "cigar.h"

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
void dumpPositions(const char *name, positionType *positions, size_t num, size_t bdSizeBytes, size_t flushEvery) {
  if (name) {
    FILE *fp = fopen(name, "wt");
    if (!fp) {
      perror(name); return;
    }
    for (size_t i = 0; i <num; i++) {
      char action = positions[i].action;
      if (action == 0) action = ' ';
      fprintf(fp, "%8d\t%10zd\t%.2lf GiB\t%.1lf%%\t%c\t%zd\t%zd\t%.2lf GiB\n", positions[i].fd, positions[i].pos, TOGiB(positions[i].pos), positions[i].pos * 100.0 / bdSizeBytes, action, positions[i].len, bdSizeBytes, TOGiB(bdSizeBytes));
      if (flushEvery && (i >= flushEvery)) {
        if ((i % flushEvery) == 0) {
          fprintf(fp, "%8d\t%10zd\t%.2lf GiB\t%.1lf%%\t%c\t%zd\t%zd\t%.2lf GiB\n", positions[i].fd, (size_t)0, 0.0, 0.0, 'F', (size_t)0, bdSizeBytes, 0.0);
        }
      }
    }
    fclose(fp);
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
		    const size_t startAtZero,
		    const size_t actualBlockDeviceSize2,
		    const int blockOffset,
		    cigartype *cigar
		    ) {
  assert(lowbs <= bs);
  assert(positions);
  assert(fdArray);

  if (bdSizeBytes < bs) {
    fprintf(stderr, "*warning* size of device is less than block size!\n");
    return;
  }

  if (startAtZero != -1) {
    fprintf(stderr,"*info* startAtZero is %zd\n", startAtZero);
  }

  // list of possibles positions
  positionType *poss = NULL, *poss2 = NULL;
  size_t possAlloc = 1024*10, count = 0, totalLen = 0;
  CALLOC(poss, possAlloc, sizeof(positionType));

  const int alignbits = (int)(log(alignment)/log(2) + 0.01);    assert((1<<alignbits) == alignment);


  // setup the start positions for the parallel files
  // with a random starting position, -z sets to 0
  size_t *positionsStart, *origStart;
  const int toalloc = (sf == 0) ? 1 : abs(sf);
    //  if (toalloc < 1) toalloc = 1;
  CALLOC(positionsStart, toalloc, sizeof(size_t));
  CALLOC(origStart, toalloc, sizeof(size_t));
  size_t maxBlock = (bdSizeBytes / bs) - 1;
  if (maxBlock < 1) maxBlock = 1;
  const size_t maxBlockBytes = maxBlock * bs;
  if (verbose >= 1) {
    fprintf(stderr,"*info* maxBlockBytes: %zd\n", maxBlockBytes);
  }
  size_t stBlock = lrand48() % (maxBlock + 1); //0..maxblock
  if ((sf == 0) || (startAtZero != -1)) {
    stBlock = startAtZero; // start from 0
  }
  
  const double blockGap = (maxBlock + 1) * 1.0 / toalloc; // need to be a floating point for large -s
  if (blockGap < 1) {
    fprintf(stderr,"*error* can't generate that many sequences %d\n", toalloc);
    exit(-1);
  }
  const size_t blockGapBytes = blockGap * bs;
  if (verbose >= 2) {
    fprintf(stderr,"*info* maxBlockBytes %zd, blockGapBytes %zd (bs %zd)\n", maxBlockBytes, blockGapBytes, bs); 
  }

  for (size_t i = 0; i < toalloc; i++) {
    positionsStart[i] = (stBlock) + (size_t) (i * blockGap); // initially in block
    positionsStart[i] = positionsStart[i] * bs;     // now in bytes
    if (positionsStart[i] > maxBlockBytes) {
      positionsStart[i] -= maxBlockBytes;
    }
    origStart[i] = positionsStart[i];
    if (verbose >= 2) {
      assert(positionsStart[i] <= maxBlockBytes);
      fprintf(stderr,"*info* alignment start %zd: %zd\n", i, positionsStart[i]);
    }
    if (verbose >= 2)
      fprintf(stderr,"*info* positionStart[%zd]: %zd\n", i, positionsStart[i]);
  }

  //    fprintf(stderr,"sf %d, %zd\n", sf, origStart[0]);
  // setup the -P positions
  while (count < *num) {
    const size_t thislen = randomBlockSize(lowbs, bs, alignbits);
    //    fprintf(stderr,"%zd   %zd.... %zd\n", origStart[(count) % toalloc], origStart[(count+1) % toalloc], poss[count].pos);

    if (count >= possAlloc) {
      possAlloc = possAlloc * 4 / 3 + 1; // grow by a 1/3 each time
      poss2 = realloc(poss, possAlloc * sizeof(positionType));
      if (!poss) {fprintf(stderr,"OOM: breaking from setup array\n");break;}
      else {
	if (verbose >= 2) {
	  fprintf(stderr,"*info*: new position size %.1lf MB array\n", TOMiB(possAlloc * sizeof(positionType)));
	}
	poss = poss2; // point to the new array
      }
    }
    
    poss[count].len = thislen;
    poss[count].pos = positionsStart[count % toalloc];

    if (abs(sf) <= 1) {
      if (poss[count].pos > maxBlockBytes) {
	if (verbose) {
	  fprintf(stderr,"*warning* got to the end, count %zd\n", count);
	}
	//	break;
      }
    }

    if (poss[count].pos > maxBlockBytes) { // wrap
      poss[count].pos -= maxBlockBytes;
    }

    assert(poss[count].pos <= maxBlockBytes);
    //poss[count].pos = poss[count].pos % (maxBlock * bs);

    
    
    //    poss[count].pos = (poss[count].pos >> alignbits) << alignbits; // check aligned
    //    assert (poss[count].pos % alignment == 0);

    size_t l = 0;
    if (sf >= 0) {
      // +ve sequential
      l = positionsStart[count % toalloc];
      l += thislen;
      l += (jumpStep * lowbs);
      while (l >= (maxBlock * bs)) {
	l -= (maxBlock * bs);
	//	l = 0;
      }
    } else {
      // negative sequential
      l = positionsStart[count % toalloc];
      if (l >= thislen) {
	l -= thislen;
      } else {
	l = (maxBlock * bs);
      }

      if (l >= jumpStep * lowbs) {
	l -= (jumpStep * lowbs);
      } else {
	l = (maxBlock * bs) - (jumpStep * lowbs);
      }
    }
    assert(l>=0);
    assert(l <= maxBlock * bs);
    positionsStart[count % toalloc] = (size_t) l;
    
    totalLen += thislen;
    if (totalLen > maxBlock * bs) { // if total length written is too much
      break;
    }

    /*
    if (abs(sf) >= 2) { // if more than two parallel regions, check they don't overlap with the next one
      size_t thispos = positionsStart[count % toalloc];
      size_t dontgo;
      if (sf > 0)
	dontgo = origStart[(count+1) % toalloc];
      else 
	dontgo = origStart[(toalloc+count-1) % toalloc];
      
      size_t diff = DIFF(thispos, dontgo);
      if (diff ==0) {
	if (verbose >= 2) {
	  fprintf(stderr,"*info* breaking after %zd positions, %zd %zd (diff %zd)\n", count, thispos, dontgo, diff);
	}
	break;
      }
    }
    */

    
    count++;
  } // find a position across the disks

  if (verbose >= 1) {
    fprintf(stderr,"*info* %zd unique positions, max %zd positions requested (-P), %.2lf GiB of device covered (%.0lf%%)\n", count, *num, TOGiB(totalLen), 100.0*TOGiB(totalLen)/TOGiB(maxBlock * bs));
  }
  if (*num > count) {
    if (verbose > 1) {
      fprintf(stderr,"*warning* there are %zd unique positions on the device\n", count);
    }
    *num = count;
  }

  // distribute among multiple FDs, do block offset
  size_t fdPos = 0;
  for (size_t i = 0; i < *num; i++) {
    positions[i].pos = poss[i].pos;
    if (blockOffset < 0) {
      if (positions[i].pos >= (-blockOffset*bs) ) {
	positions[i].pos += (blockOffset*bs);
      } else {
	// wrap
	//	fprintf(stderr,"wrap %zd\n", positions[i].pos);
	positions[i].pos = (maxBlock * bs) - positions[i].pos + (blockOffset * bs);
      }
    } else {
      positions[i].pos = poss[i].pos + (blockOffset*bs);
    }
    positions[i].len = poss[i].len;
    positions[i].fd = fdArray[fdPos++];
    if (fdPos >= fdSize) fdPos = 0;
  }
  
  // if randomise then reorder
  if (/*(0) &&*/ (sf == 0)) {
    for (size_t shuffle = 0; shuffle < 1; shuffle++) {
      if (verbose >= 1) {
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
	positionType p = positions[i];
	positions[i] = positions[j];
	positions[j] = p;
      }
    }
  }

  // setup R/W
  positionType *p = positions;
  size_t cl = 0;
  if (cigar) cl = cigar_len(cigar);
  
  for (size_t j = 0; j < *num; j++) {
    char action;
    if (cl > 0) { // if we have a CIGAR
      if (sf != 0) {
	action = cigar_at(cigar, (j / ABS(sf)) % cl); // have the exact the same pattern per contiguous range
      } else {
	action = cigar_at(cigar, j % cl);
      }
    } else {
      action = 'X';
    }
    
    if (action == 'X') {
      if (drand48() <= readorwrite) {
	p->action = 'R';
      } else {
	p->action = 'W';
      }
    } else if (action == 'S') {
      p->action = action; // skip
    } else if (action == 'B') {
      // backwards
      p->action = positions[j-1].action;
      p->pos = positions[j-1].pos;
      p->len = positions[j-1].len;
    } else if (action == 'R' || action == 'W') {
      p->action = action;
    } else {
      fprintf(stderr,"*error* unknown CIGAR action '%c'\n", action);
      exit(-1);
    }
    p->success = 0;
    assert(p->len); // can't be zero
    assert(p->len >= lowbs);
    assert(p->len <= bs);
    p++;
  }

  if (verbose >= 1) {
    fprintf(stderr,"*info* unique positions: %zd\n", *num);
    for (size_t i = 0; i < MIN(*num, 30); i++) {
      fprintf(stderr,"*info* [%zd]:\t%14zd\t%14.2lf GB\t%8zd\t%c\t%d\n", i, positions[i].pos, TOGiB(positions[i].pos), positions[i].len, positions[i].action, positions[i].fd);
    }
  }


  free(poss); // free the possible locations
}


/*int main() {
  const size_t num = 10*1024*1024;
  positionType *p = createPositions(num);

  size_t bdSizeBytes = 2uL*1024uL*1024uL*1024uL -1;
  createLinearPositions(p, num, 65536, bdSizeBytes, 0, 10000000, 1.0, 1);
  createParallelPositions(p, num, 65536, bdSizeBytes, 10, 1.0, 10);

  //  checkPositionArray(p, num, bdSizeBytes);
  dumpPositions(p, num, bdSizeBytes);
  }*/
