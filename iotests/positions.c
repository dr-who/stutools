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
  if (num < 1 || num > ((size_t)-1) / 2) {
    fprintf(stderr,"*warning* createPositions num was 0?\n");
    num=1;
  }
  CALLOC(p, num, sizeof(positionType));
  return p;
}

void freePositions(positionType *p) {
  free(p);
  p = NULL;
}


int checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes) {
  //  fprintf(stderr,"... checking\n");
  size_t rcount = 0, wcount = 0;
  size_t sizelow = -1, sizehigh = 0;
  positionType *p = (positionType *)positions;
  for (size_t j = 0; j < num; j++) {
    if (p->len == 0) {
      return 1;
    }
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

  if (sizelow <= 0) {
    return 1;
  }
  // check all positions are aligned to low and high lengths
  p = (positionType *) positions;
  if (sizelow > 0) {
    for (size_t j = 0; j < num; j++) {
      if (p->len == 0) {
	fprintf(stderr,"*error* len of 0\n"); return 1;
      }
      if ((p->len %sizelow) != 0) {
	fprintf(stderr,"*error* len not aligned\n"); return 1;
      }
      if (p->pos + sizehigh > bdSizeBytes) {
	fprintf(stderr,"*error* off the end of the array %zd!\n", p->pos); return 1;
      }
      if ((p->pos % sizelow) != 0) {
	fprintf(stderr,"*error* no %zd aligned position at %zd!\n", sizelow, p->pos); return 1;
      }
      if ((p->pos % sizehigh) != 0) {
	fprintf(stderr,"*error* no %zd aligned position at %zd!\n", sizehigh, p->pos); return 1;
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
  return 0;
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
void setupPositions1(positionType *positions,
		    size_t *num,
		     const int fd,
		    const size_t bdSizeBytes,
		    const int sf,
		    const double readorwrite,
		    const size_t lowbs,
		    const size_t bs,
		    size_t alignment,
		    const size_t singlePosition,
		    const int    jumpStep,
		    const long startingBlock,
		    const size_t actualBlockDeviceSize,
		    const int blockOffset,
		    cigartype *cigar
		    ) {
  assert(lowbs <= bs);
  assert(positions);

  if (bdSizeBytes < bs) {
    fprintf(stderr, "*warning* size of device is less than block size!\n");
    return;
  }

  if (*num == 0) {
    fprintf(stderr,"*error* setupPositions number of positions can't be 0\n");
  }

  if (verbose) {
    if (startingBlock != -99999) {
      fprintf(stderr,"*info* startingBlock is %ld on fd %d\n", startingBlock, fd);
    }
  }

  // list of possibles positions
  positionType *poss = NULL, *poss2 = NULL;
  size_t possAlloc = 1024*10, count = 0, totalLen = 0;
  CALLOC(poss, possAlloc, sizeof(positionType));

  const int alignbits = (int)(log(alignment)/log(2) + 0.01);
  if (1<<alignbits != alignment) {
    fprintf(stderr,"*error* alignment of %zd not suitable, changing to %d\n", alignment, 1<<alignbits);
    alignment = 1<< alignbits;
  }//assert((1<<alignbits) == alignment);


  // setup the start positions for the parallel files
  // with a random starting position, -z sets to 0
  size_t *positionsStart, *origStart;
  const int toalloc = (sf == 0) ? 1 : abs(sf);
    //  if (toalloc < 1) toalloc = 1;
  CALLOC(positionsStart, toalloc, sizeof(size_t));
  CALLOC(origStart, toalloc, sizeof(size_t));
  size_t maxBlockIncl = (bdSizeBytes / bs) - 1;
  //  assert (startBlock <= maxBlockIncl);
  if (maxBlockIncl < 1) maxBlockIncl = 1;
  const size_t byteSeekMaxLoc = maxBlockIncl * bs; // [0..maxBlockIncl] inclusive
  if (verbose >= 1) {
    fprintf(stderr,"*info* byteSeekMaxLoc: %zd, blockSeek [0..%zd], bdSizeBytes %zd (%.2lf GiB)\n", byteSeekMaxLoc, maxBlockIncl, bdSizeBytes, TOGiB(bdSizeBytes));
  }
  long stBlock = 0;
  if (startingBlock == -99999) {
    stBlock = lrand48() % (maxBlockIncl + 1); //0..maxblock, if it's not -99999 then set it.
    if (verbose) {
      fprintf(stderr,"*info* setting random block start to block %ld\n", stBlock);
    }
  } else if (startingBlock < 0) {
    stBlock = maxBlockIncl + 1 + startingBlock;
  } else {
    stBlock = startingBlock; // start from 0
  }
  stBlock = stBlock % (maxBlockIncl+1);
  assert (stBlock <= maxBlockIncl);
  assert (stBlock >= 0);
  
  const double blockGap = (maxBlockIncl + 1) * 1.0 / toalloc; // need to be a floating point for large -s
  if (blockGap < 1) {
    fprintf(stderr,"*error* can't generate that many sequences %d\n", toalloc);
    exit(-1);
  }

  for (size_t i = 0; i < toalloc; i++) {
    positionsStart[i] = (stBlock) + (size_t) (i * blockGap); // initially in block
    positionsStart[i] = positionsStart[i] % (maxBlockIncl + 1); // don't seek past maxBlockIncl
    positionsStart[i] = positionsStart[i] * bs; // now in bytes
    origStart[i] = positionsStart[i]; // copy of start

    assert(positionsStart[i] <= byteSeekMaxLoc);
    if (verbose >= 2) {
      fprintf(stderr,"*info* alignment start %zd: %zd (block %zd/max %zd)\n", i, positionsStart[i], positionsStart[i] / bs, maxBlockIncl);
    }
  }

  // setup the -P positions

  while (count < *num) {
    const size_t thislen = randomBlockSize(lowbs, bs, alignbits);
    assert(thislen >= lowbs);
    assert(thislen <= bs);
    assert(((size_t)(thislen / 2)) * 2 == thislen);

    if (count >= possAlloc) {
      possAlloc = possAlloc * 2 + 1; // grow by a 1/3 each time
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
    poss[count].fd = fd;
    poss[count].pos = positionsStart[count % toalloc];

    long l = 0;
    if (sf >= 0) {
      // +ve sequential
      l = positionsStart[count % toalloc];
      if (jumpStep >= 0) {
	// +ve jumping
	l += thislen;
	l += (jumpStep * lowbs);
	if (l > byteSeekMaxLoc) {
	  l = l - byteSeekMaxLoc - lowbs;
	  //l = l % bdSizeBytes;
	}
	assert(l>=0);
	//	      fprintf(stderr,"l %zd, bsm %zd (bdSize %zd), maxBlock*bs %zd\n", l, byteSeekMaxLoc, bdSizeBytes, maxBlockIncl * bs);
	assert(l <= maxBlockIncl * bs);

      } else {
	// -ve jumping
	if (l >= thislen * ABS(jumpStep)) {
	  l -= (thislen * ABS(jumpStep));
	} else {
	  //	    fprintf(stderr,"wrapping %zd6\n", l);
	  l = l + bdSizeBytes - (ABS(jumpStep))*bs;
	  //	    l = (maxBlockIncl * bs) + bs - thislen; //
	}
      }
      assert(l>=0);
      //      fprintf(stderr,"l %zd, bsm %zd (bdSize %zd), maxBlock*bs %zd\n", l, byteSeekMaxLoc, bdSizeBytes, maxBlockIncl * bs);
      assert(l <= maxBlockIncl * bs);

    } else {
      // negative sequential
      l = positionsStart[count % toalloc];
      if (l >= thislen) {
	fprintf(stderr,"underflow4\n");
	l -= thislen;
      } else {
	fprintf(stderr,"underflow\n");
	l = (maxBlockIncl * bs) + bs - thislen; // go back to max seek position
      }
	
      if (l >= ABS(jumpStep) * lowbs) {
	fprintf(stderr,"underflow3\n");
	l -= (ABS(jumpStep) * lowbs);
      } else {
	fprintf(stderr,"underflow2\n");
	l = (maxBlockIncl * bs) + bs - thislen;
      }
    }
    assert(l>=0);
    //      fprintf(stderr,"l %zd, bsm %zd (bdSize %zd), maxBlock*bs %zd\n", l, byteSeekMaxLoc, bdSizeBytes, maxBlockIncl * bs);
    assert(l <= maxBlockIncl * bs);
    positionsStart[count % toalloc] = (size_t) l;
      
	
    totalLen += thislen;
    if (totalLen >= ((maxBlockIncl +1) * bs)) { // if total length written is too much
      break;
    }


    count++;
  } // find a position across the disks

  if (verbose >= 1) {
    fprintf(stderr,"*info* %zd unique positions, max %zd positions requested (-P), %.2lf GiB of device covered (%.0lf%%)\n", count, *num, TOGiB(totalLen), 100.0*TOGiB(totalLen)/TOGiB(maxBlockIncl * bs));
  }
  if (*num > count) {
    if (verbose > 1) {
      fprintf(stderr,"*warning* there are %zd unique positions on the device\n", count);
    }
    *num = count;
  }

  // copy to positions
  for (size_t i = 0; i < *num ; i++) {
    positions[i] = poss[i];
  }
  //  }*/
  
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


  // p left, thepos right
  size_t thepos = 0; // position
  
  
  for (size_t j = 0; j < *num; j++) { // do the right number
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

    //    fprintf(stderr,"pre j %zd, action %c, index %zd, pos %zd\n", p - positions, action, thepos, positions[thepos].pos);
    
    if (action == 'X') {
      if (drand48() <= readorwrite) {
	p->action = 'R';
      } else {
	p->action = 'W';
      }
      p->pos = positions[thepos].pos;
      p->len = positions[thepos].len;
      assert(p->len); // can't be zero
      assert(p->len >= lowbs);
      assert(p->len <= bs);
      p->success = 0;
      p++;
      thepos++;
      //    } else if (action == 'S') {
      //      thepos++;
    } else if (action == 'B') {
      if (thepos > 1) thepos--;
    } else if (action == 'R' || action == 'W' || action == 'S') {
      p->action = action;
      p->pos = positions[thepos].pos;
      p->len = positions[thepos].len;
      assert(p->len); // can't be zero
      assert(p->len >= lowbs);
      assert(p->len <= bs);
      p->success = 0;
      p++;
      thepos++;
    } else {
      fprintf(stderr,"*error* unknown CIGAR action '%c'\n", action);
      exit(-1);
    }
    //    fprintf(stderr,"  post j %zd, action %c, index %zd, pos %zd\n", p- positions, action, thepos, positions[thepos].pos);
    if (thepos >= *num) {
      thepos = 0;
    }

    //    assert(p->pos <= byteSeekMaxLoc);
    assert(thepos <= *num);
    assert(p -positions <= *num);
  }

  /*  if (verbose >= 1) {
    fprintf(stderr,"*info* unique positions: %zd\n", *num);
    for (size_t i = 0; i < MIN(*num, 30); i++) {
      fprintf(stderr,"*info* [%zd]:\t%14zd\t%14.2lf GB\t%8zd\t%c\t%d\n", i, positions[i].pos, TOGiB(positions[i].pos), positions[i].len, positions[i].action, positions[i].fd);
    }
    }*/


  free(poss); // free the possible locations
  free(positionsStart);
  free(origStart);
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
		    size_t alignment,
		    const size_t singlePosition,
		    const int    jumpStep,
		    const long startingBlock,
		    const size_t actualBlockDeviceSize,
		    const int blockOffset,
		    cigartype *cigar
		    ) {
  positionType **flatpositions;
  size_t *flatnum;
  CALLOC(flatpositions, fdSize, sizeof(positionType *));
  CALLOC(flatnum, fdSize, sizeof(size_t));
  
  flatnum[0] = *num;
  flatpositions[0] = createPositions(flatnum[0]);
  assert(flatpositions[0]);
  setupPositions1(flatpositions[0], &(flatnum[0]), fdArray[0], bdSizeBytes, sf, readorwrite, lowbs, bs, alignment, singlePosition, jumpStep, startingBlock, actualBlockDeviceSize, blockOffset, cigar);
  
  for (size_t i = 1; i < fdSize; i++) {
    flatnum[i] = flatnum[0];
    flatpositions[i] = createPositions(flatnum[i]);
    for (size_t j = 0; j < flatnum[i]; j++) {
      flatpositions[i][j] = flatpositions[0][j];
      flatpositions[i][j].fd = fdArray[i];
    }
  }

  // merge them
  size_t outcount = 0;
  //  fprintf(stderr,"starting num %zd\n", *num);
  for (size_t i = 0; i < *num; i++) {
    for (size_t fd = 0; fd < fdSize; fd++) {
      positions[outcount++] = flatpositions[fd][i];
      if ((outcount >= *num) || (outcount >= flatnum[fd] * fdSize)) {
	//	fprintf(stderr,"*warning* finished at %zd\n", flatnum[fd]);
	fd = fdSize;
	i = *num;
	break;
      }
    }
  }
      
  *num = outcount;
  //  fprintf(stderr,"num = %zd\n", *num);
  
  for (size_t i = 0; i < fdSize; i++) {
    freePositions(flatpositions[i]);
  }
  free(flatnum);
  free(flatpositions);

  if (verbose >= 1) {
    fprintf(stderr,"*info* unique positions: %zd\n", *num);
    for (size_t i = 0; i < MIN(*num, 30); i++) {
      fprintf(stderr,"*info* [%zd]:\t%14zd\t%14.2lf GB\t%8zd\t%c\t%d\n", i, positions[i].pos, TOGiB(positions[i].pos), positions[i].len, positions[i].action, positions[i].fd);
    }
  }



}



void simpleSetupPositions(positionType *positions,
			  size_t *num,
			  const int *fdArray,
			  const size_t fdSize,
			  const long startingBlock,
			  const size_t bdSizeBytes,
			  const size_t bs)
{
  setupPositions(positions, num, fdArray, fdSize, bdSizeBytes,
		 1, // sequential single
		 1, // read
		 bs, bs, // block range
		 bs, // alignment
		 0,
		 0,
		 startingBlock,
		 bdSizeBytes,
		 0,
		 NULL);
}

		 
		 
