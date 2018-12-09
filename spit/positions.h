#ifndef _POSITIONS_H
#define _POSITIONS_H

#include "cigar.h"
#include "devices.h"

typedef struct {
  size_t pos;          // 8
  double submittime, finishtime; // 8
  unsigned int len;  // 4;
  unsigned short seed; // 2
  char  action;        // 1: 'R' or 'W'
  char  success;       // 1 
} positionType;

typedef struct {
  positionType *positions;
  size_t sz;
  char *string;
  char *device;
} positionContainer;

positionType *createPositions(size_t num);

int checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes);
void savePositions(const char *name, positionType *positions, size_t num, size_t flushEvery);

positionType *loadPositions(FILE *fd, size_t *num, deviceDetails **devs, size_t *numDevs, size_t *maxsize);

void infoPositions(const deviceDetails *devList, const size_t devCount);

void setupPositions(positionType *positions,
		    size_t *num,
		    const int sf,
		    const double readorwrite,
		    const size_t lowbs,
		    const size_t bs,
		    size_t alignment,
		    const long startingBlock,
		    const size_t actualBlockDeviceSize,
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

void findSeedMaxBlock(positionType *positions, const size_t num, long *seed, size_t *minbs, size_t *blocksize);

void dumpPositions(positionType *positions, const char *prefix, const size_t num, const size_t countToShow);

void positionContainerInit(positionContainer *pc);
void positionContainerSetup(positionContainer *pc, size_t sz, char *device, char *string);
void positionContainerFree(positionContainer *pc);

void positionContainerSave(const positionContainer *pc, const char *name, const size_t flushEvery);
void positionContainerLoad(positionContainer *pc, const char *fn);

void positionLatencyStats(positionContainer *pc, const int threadid);

void setupRandomPositions(positionType *pos,
			  const size_t num,
			  const double rw,
			  const size_t bs,
			  const size_t highbs,
			  const size_t alignment,
			  const size_t bdSize,
			  const size_t seedin);

#endif

