#ifndef _POSITIONS_H
#define _POSITIONS_H

#include "cigar.h"
#include "devices.h"

typedef struct {
  size_t pos;
  char   action; // 'R' or 'W'
  size_t len;
  int    success;
  long   seed;
  deviceDetails *dev;
} positionType;

positionType *createPositions(size_t num);

int checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes);
void savePositions(const char *name, positionType *positions, size_t num, size_t flushEvery);

positionType *loadPositions(FILE *fd, size_t *num, deviceDetails **devs, size_t *numDevs);

void infoPositions(const deviceDetails *devList, const size_t devCount);

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
		    const size_t actualBlockDeviceSize,
		    const int blockOffset,
		    cigartype *cigar,
		    long seed);

void simpleSetupPositions(positionType *positions,
			  size_t *num,
			  deviceDetails *devList,
			  const size_t devCount,
			  const long startingBlock,
			  const size_t bdSizeBytes,
			  const size_t bs);



void freePositions(positionType *p);
void positionStats(const positionType *positions, const size_t num, const deviceDetails *devList, const size_t devCount);

void findSeedMaxBlock(positionType *positions, const size_t num, long *seed, size_t *blocksize);

#endif

