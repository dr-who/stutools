#ifndef _POSITIONS_H
#define _POSITIONS_H

#include "cigar.h"

typedef struct {
  int    fd;
  size_t pos;
  char   action; // 'R' or 'W'
  size_t len;
  int    success;
} positionType;

positionType *createPositions(size_t num);

int checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes);
void dumpPositions(const char *name, positionType *positions, size_t num, size_t bdSizeBytes, size_t flushEvery);

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
		    const long startingBlock,
		    const size_t actualBlockDeviceSize,
		    const int blockOffset,
		    cigartype *cigar);

void simpleSetupPositions(positionType *positions,
			  size_t *num,
			  const int *fdArray,
			  const size_t fdSize,
			  const long startingBlock,
			  const size_t bdSizeBytes,
			  const size_t bs);



void freePositions(positionType *p);


#endif

