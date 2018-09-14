#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <math.h>

#include "devices.h"
#include "utils.h"
#include "positions.h"
#include "cigar.h"

extern int verbose;
extern int keepRunning;

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
  if (num == 0) {
    fprintf(stderr,"*warning* createPositions num was 0?\n");
    return NULL;
  }
  //fprintf(stderr,"create positions %zd\n", num);
  CALLOC(p, num, sizeof(positionType));
  return p;
}

void freePositions(positionType *p) {
  free(p);
  p = NULL;
}


int checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes) {
  if (verbose) {
    fprintf(stderr,"*info*... checking position array with %zd values...\n", num);
  }
  
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
    abort();
    //    return 1;
  }
  // check all positions are aligned to low and high lengths
  p = (positionType *) positions;
  if (sizelow > 0) {
    for (size_t j = 0; j < num; j++) {
      if (p->len == 0) {
	fprintf(stderr,"*error* len of 0\n"); 
      }
      //      if ((p->len %sizelow) != 0) {
      //	fprintf(stderr,"*error* len not aligned\n"); 
      //      }
      if (p->pos + p->len > bdSizeBytes) {
	fprintf(stderr,"*error* off the end of the array %zd + %d is %zd (%zd)!\n", p->pos, p->len, p->pos + p->len, bdSizeBytes); 
      }
      //      if ((p->pos % sizelow) != 0) {
      //	fprintf(stderr,"*error* no %zd aligned position at %zd!\n", sizelow, p->pos); 
      //      }
      //      if ((p->pos % sizehigh) != 0) {
      //	fprintf(stderr,"*error* no %zd aligned position at %zd!\n", sizehigh, p->pos); 
      //      }
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
    //    fprintf(stderr,"-----%zd %zd\n", i, copy[i].pos);
    if (copy[i].pos != copy[i-1].pos) {
      unique++;
      if (i==1) { // if the first number checked is different then really it's 2 unique values.
	unique++;
      }
    }
    if (copy[i].pos < copy[i-1].pos) {
      fprintf(stderr,"not sorted %zd %zd, unique %zd\n",i, copy[i].pos, unique);
      abort();
    }
    if (copy[i-1].pos + copy[i-1].len > copy[i].pos) {
      fprintf(stderr,"eerk overlapping %zd %zd %d \n",i, copy[i].pos, copy[i].len);
      abort();
    }
  }
  free(copy);

  if (verbose)
    fprintf(stderr,"*info* action summary: reads %zd, writes %zd, checked ratio %.1lf, len [%zd, %zd], unique positions %zd\n", rcount, wcount, rcount*1.0/(rcount+wcount), sizelow, sizehigh, unique);
  return 0;
}


// lots of checks
void savePositions(const char *name, positionType *positions, size_t num, size_t flushEvery) {
  if (name) {
    FILE *fp = fopen(name, "wt");
    if (!fp) {
      perror(name); return;
    }
    for (size_t i = 0; i <num; i++) {
      if (positions[i].success) {
	size_t bdSizeBytes = positions[i].dev->bdSize;
	const char action = positions[i].action;
	if (action == 'R' || action == 'W') {
	  fprintf(fp, "%s\t%10zd\t%.2lf GiB\t%.1lf%%\t%c\t%u\t%zd\t%.2lf GiB\t%ld\n", positions[i].dev->devicename, positions[i].pos, TOGiB(positions[i].pos), positions[i].pos * 100.0 / bdSizeBytes, action, positions[i].len, bdSizeBytes, TOGiB(bdSizeBytes), positions[i].seed);
	}
	if (flushEvery && ((i+1) % (flushEvery) == 0)) {
	  fprintf(fp, "%s\t%10zd\t%.2lf GiB\t%.1lf%%\t%c\t%zd\t%zd\t%.2lf GiB\t%ld\n", positions[i].dev->devicename, (size_t)0, 0.0, 0.0, 'F', (size_t)0, bdSizeBytes, 0.0, positions[i].seed);
	}
      }
    }
    fclose(fp);
  }
}



// create the position array
void setupPositions1(positionType *positions,
		    size_t *num,
		    deviceDetails *dev,
		    const size_t bdSizeBytes,
		    const int sf,
		    const double readorwrite,
		    const size_t lowbs,
		    const size_t bs,
		    size_t alignment,
		    const size_t singlePosition,
		    const int    jumpStep,
		    const long startingBlock,
		    const size_t bdSizeTotal,
		    const int blockOffset,
		    cigartype *cigar,
		    long seed
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
      fprintf(stderr,"*info* startingBlock is %ld\n", startingBlock);
    }
  }

  // list of possibles positions
  positionType *poss = NULL;
  size_t possAlloc = 1024*10, count = 0, totalLen = 0;
  CALLOC(poss, possAlloc, sizeof(positionType));

  const int alignbits = (int)(log(alignment)/log(2) + 0.01);
  //  if (1<<alignbits != alignment) {
  //    fprintf(stderr,"*error* alignment of %zd not suitable, changing to %d\n", alignment, 1<<alignbits);
  //    alignment = 1<< alignbits;
  //  }//assert((1<<alignbits) == alignment);

  // setup the start positions for the parallel files
  // with a random starting position, -z sets to 0
  size_t *positionsStart, *positionsEnd;
  const int toalloc = (sf == 0) ? 1 : abs(sf);
  CALLOC(positionsStart, toalloc, sizeof(size_t));
  CALLOC(positionsEnd, toalloc, sizeof(size_t));
  
  for (size_t i = 0; i < toalloc; i++) {
    positionsStart[i] = alignedNumber(i * (bdSizeBytes / toalloc), lowbs); 
    if (i > 0) positionsEnd[i-1] = positionsStart[i];
    positionsEnd[toalloc-1] = bdSizeBytes;

    if (verbose >= 2) {
      fprintf(stderr,"*info* alignment start %zd: %zd (block %zd)\n", i, positionsStart[i], positionsStart[i] / bs);
    }
  }

  // setup the -P positions

  // do it nice
  count = 0;
  while (count < *num) {
    int nochange = 1;
    for (size_t i = 0; i < toalloc; i++) {
      size_t j = positionsStart[i]; // while in the range
      if (j < positionsEnd[i]) {
	const size_t thislen = randomBlockSize(lowbs, bs, alignbits);
	assert(thislen >= 0);

	// grow destination array
	if (count >= possAlloc) {
	  possAlloc = possAlloc * 2 + 1; // grow by a 1/3 each time
	  positionType *poss2 = realloc(poss, possAlloc * sizeof(positionType));
	  if (!poss) {fprintf(stderr,"OOM: breaking from setup array\n");break;}
	  else {
	    if (verbose >= 2) {
	      fprintf(stderr,"*info*: new position size %.1lf MB array\n", TOMiB(possAlloc * sizeof(positionType)));
	    }
	    poss = poss2; // point to the new array
	  }
	}

	// if we have gone over the end of the range
	if (j + thislen > positionsEnd[i]) {positionsStart[i] += thislen; break;}

	poss[count].pos = j;
	poss[count].len = thislen;
	assert(poss[count].len >= 0);
	poss[count].dev = dev;
	poss[count].seed = seed;
	
	positionsStart[i] += thislen;
	
	count++;
	nochange = 0;
	if (count >= *num) break; // if too many break
      }
    }
    if (nochange) break;
  }
  if (count < *num) {
    if (verbose > 1) {
      fprintf(stderr,"*warning* there are %zd unique positions on the device\n", count);
    }
  }
  *num = count;

  // make a complete copy and rotate by an offset

  int offset = 0;
  if (count) {
    if (startingBlock == -99999) {
      offset = (lrand48() % count);
    } else {
      offset = startingBlock % count;
    }
  }

  // rotate
  for (size_t i = 0; i < count; i++) {
    int index = i + offset;
    if (index >= count) {
      index -= count;
    }
    positions[i] = poss[index];
    if (drand48() <= readorwrite) positions[i].action='R'; else positions[i].action='W';
    assert(positions[i].len >= 0);
  }

  if (verbose >= 1) {
        fprintf(stderr,"*info* %zd unique positions, max %zd positions requested (-P), %.2lf GiB of device covered (%.0lf%%)\n", count, *num, TOGiB(totalLen), 100.0*TOGiB(totalLen)/TOGiB(bdSizeTotal));
  }
  
  // if randomise then reorder
  if (sf == 0) {
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

  // rotate
  for (size_t i = 0; i < *num; i++) {
    assert(positions[i].len >= 0);
  }

  
  if (0) 
    for (size_t j = 0; j < *num; j++) { // do the right number
      /*
      size_t cl = 0;
      if (cigar) cl = cigar_len(cigar);
      
      
      for (size_t i = 0; i < *num; i++) {
	assert(positions[i].len >= 0);
	}*/

      /*      char action;
      if (cl > 0) { // if we have a CIGAR
	if (sf != 0) {
	  action = cigar_at(cigar, (j / ABS(sf)) % cl); // have the exact the same pattern per contiguous range
	} else {
	  action = cigar_at(cigar, j % cl);
	}
      } else {
	action = 'X';
	}*/

      //    fprintf(stderr,"%zd %d action %c\n", j, positions[j].len, action);


      //    fprintf(stderr,"pre j %zd, action %c, index %zd, pos %zd\n", p - positions, action, thepos, positions[thepos].pos);

      /*
	if (action == 'X') {
	if (drand48() <= readorwrite) {
	positions[j].action = 'R';
	} else {
	positions[j].action = 'W';
	}
	positions[j].pos = positions[thepos].pos;
	positions[j].len = positions[thepos].len;
	fprintf(stderr,"the pos j %zd , %zd, positions[j] %zd\n", j, thepos, positions[j].len);
	assert(positions[j].len); // can't be zero
	assert(positions[j].len >= lowbs);
	assert(positions[j].len <= bs);
	positions[j].success = 0;
	p++;
	thepos++;
	//    } else if (action == 'S') {
	//      thepos++;
	} else if (action == 'B') {
	if (thepos > 1) thepos--;
	} else if (action == 'R' || action == 'W' || action == 'S') {
	positions[j].action = action;
	positions[j].pos = positions[thepos].pos;
	positions[j].len = positions[thepos].len;
	assert(positions[j].len); // can't be zero
	assert(positions[j].len >= lowbs);
	assert(positions[j].len <= bs);
	positions[j].success = 0;
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

	if (verbose >= 1) {
	fprintf(stderr,"*info* unique positions: %zd\n", *num);
	for (size_t i = 0; i < MIN(*num, 30); i++) {
	fprintf(stderr,"*info* [%zd]:\t%14zd\t%14.2lf GB\t%8zd\t%c\t%d\n", i, positions[i].pos, TOGiB(positions[i].pos), positions[i].len, positions[i].action, positions[i].fd);
	}
      */
    }

      
  free(poss); // free the possible locations
  free(positionsStart);
  free(positionsEnd);
}



// create the position array
void setupPositions(positionType *positions,
		    size_t *num,
		    deviceDetails *devList,
		    const size_t devCount,
		    const int sf,
		    const double readorwrite,
		    const size_t lowbs,
		    const size_t bs,
		    size_t alignment,
		    const size_t singlePosition,
		    const int    jumpStep,
		    const long startingBlock,
		    const size_t bdSizeWeAreUsing,
		    const int blockOffset,
		    cigartype *cigar,
		    long seed
		    ) {

  if (positions) {
    positionType **flatpositions;
    size_t *flatnum;
    CALLOC(flatpositions, devCount, sizeof(positionType *));
    CALLOC(flatnum, devCount, sizeof(size_t));

    flatnum[0] = *num;
    flatpositions[0] = createPositions(flatnum[0]);
    assert(flatpositions[0]);
    assert(flatnum[0] >= 1);

    setupPositions1(flatpositions[0], &(flatnum[0]), &devList[0], bdSizeWeAreUsing, sf, readorwrite, lowbs, bs, alignment, singlePosition, jumpStep, startingBlock, bdSizeWeAreUsing, blockOffset, cigar, seed);

    checkPositionArray(flatpositions[0], flatnum[0], bdSizeWeAreUsing);
  
    for (size_t i = 1; i < devCount; i++) {
      flatnum[i] = flatnum[0];
      flatpositions[i] = createPositions(flatnum[i]);
      for (size_t j = 0; j < flatnum[i]; j++) {
	flatpositions[i][j] = flatpositions[0][j];
	flatpositions[i][j].dev = &(devList[i]);
      }
    }

    // merge them
    size_t outcount = 0;
    //      fprintf(stderr,"starting num %zd\n", *num);
    for (size_t i = 0; i < *num; i++) {
      for (size_t fd = 0; fd < devCount; fd++) {
	positions[outcount++] = flatpositions[fd][i];
	if ((outcount >= *num) || (outcount >= flatnum[fd] * devCount)) {
	  //	fprintf(stderr,"*warning* finished at %zd\n", flatnum[fd]);
	  //fd = devCount;
	  i = *num;
	  break;
	}
      }
    }
      
    *num = outcount;
  
    for (size_t i = 0; i < devCount; i++) {
      freePositions(flatpositions[i]);
    }
    free(flatnum);
    free(flatpositions);

    if (verbose >= 1) {
      fprintf(stderr,"*info* unique positions: %zd\n", *num);
      for (size_t i = 0; i < MIN(*num, 30); i++) {
	fprintf(stderr,"*info* [%zd]:\t%14zd\t%14.2lf GB\t%8u\t%c\t%s\n", i, positions[i].pos, TOGiB(positions[i].pos), positions[i].len, positions[i].action, positions[i].dev->devicename);
      }
    }
  }
}



void simpleSetupPositions(positionType *positions,
			  size_t *num,
			  deviceDetails *devList,
			  const size_t devCount,
			  const long startingBlock,
			  const size_t bdSizeBytes,
			  const size_t bs)
{
  setupPositions(positions, num, devList, devCount,
		 1, // sequential single
		 1, // read
		 bs, bs, // block range
		 bs, // alignment
		 0,
		 0,
		 startingBlock,
		 bdSizeBytes,
		 0,
		 NULL,
		 0);
}

		 
void positionStats(const positionType *positions, const size_t maxpositions, const deviceDetails *devList, const size_t devCount) {
  size_t len = 0;
  for (size_t i = 0; i < maxpositions; i++) {
    len += positions[i].len;
  }
  size_t totalBytes = 0;
  for (size_t i = 0; i <devCount; i++) {
    totalBytes += devList[i].bdSize;
  }

  fprintf(stderr,"*info* %zd positions, %.2lf GiB positions from a total of %.2lf GiB, coverage (%.0lf%%)\n", maxpositions, TOGiB(len), TOGiB(totalBytes), 100.0*TOGiB(len)/TOGiB(totalBytes));
}
    
  
positionType *loadPositions(FILE *fd, size_t *num, deviceDetails **devs, size_t *numDevs) {

  char *line = malloc(200000);
  size_t maxline = 200000;
  ssize_t read;
  char *path;
  CALLOC(path, 1000, 1);
  //  char *origline = line; // store the original pointer, as getline changes it creating an unfreeable area
  positionType *p = NULL;
  size_t pNum = 0;
  
  while ((read = getline(&line, &maxline, fd)) != -1) {
    size_t pos, len, seed;
    //    fprintf(stderr,"%zd\n", strlen(line));
    char op;
    
    int s = sscanf(line, "%s %zu %*s %*s %*s %c %zu %*s %*s %*s %zu", path, &pos, &op, &len, &seed);
    if (s >= 5) {
      //      fprintf(stderr,"%s %zd %c %zd %zd\n", path, pos, op, len, seed);
      deviceDetails *d2 = addDeviceDetails(path, devs, numDevs);
      pNum++;
      p = realloc(p, sizeof(positionType) * (pNum));
      assert(p);
      //      fprintf(stderr,"%zd\n", pNum);
      //      p[pNum-1].fd = 0;
      p[pNum-1].dev = d2;
      p[pNum-1].pos = pos;
      p[pNum-1].len = len;
      p[pNum-1].action = op;
      p[pNum-1].success = 1;
      p[pNum-1].seed = seed;

      //      fprintf(stderr,"added %p\n", p[pNum-1].dev);
      
    //    addDeviceDetails(line, devs, numDevs);
    //    addDeviceToAnalyse(line);
    //    add++;
    //    printf("%s", line);
    }
  }
  fflush(stderr);

  free(line);

  free(path);
  *num = pNum;
  return p;
}

void findSeedMaxBlock(positionType *positions, const size_t num, long *seed, size_t *minbs, size_t *blocksize) {
  *blocksize = 0;
  *minbs = 0;
  for (size_t i = 0; i < num; i++) {
    if (i==0) {
      *minbs = positions[i].len;
      *blocksize = *minbs;
    }
    *seed = positions[i].seed;
    if (positions[i].len > *blocksize) {
      *blocksize = positions[i].len;
    }
    if (positions[i].len < *minbs) {
      *minbs = positions[i].len;
    }
  }
}
    
  
