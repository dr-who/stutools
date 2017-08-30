#ifndef _POSITIONS_H
#define _POSITIONS_H

typedef struct {
  size_t pos;
  char   action; // 'R' or 'W'
  size_t len;
  int    success;
} positionType;

positionType *createPositions(size_t num);

void checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes);
void dumpPositions(positionType *positions, size_t num, size_t bdSizeBytes);

void createLinearPositions(positionType *positions,
			   const size_t numPositions,
			   const size_t blockSize,
			   const size_t bdSizeBytes,     // max block device size in bytes
			   const size_t startBlock, // start block * bdSize
			   const size_t finBlock,   // finis block * bdSize
			   const size_t jumpBlock,
			   const double rwRatio);

void createParallelPositions(positionType *positions,
			     const size_t numPositions,
			     const size_t blockSize,
			     const size_t bdSizeBytes,
			     const size_t jumpBlock,
			     const double rwRatio,
			     const size_t parallelRegions);

void setupPositions(positionType *positions,
		    const size_t num,
		    const size_t bdSizeBytes,
		    const int sf,
		    const double readorwrite,
		    const size_t bs,
		    const size_t singlePosition,
		    const size_t jumpStep,
		    const size_t startAtZero);



#endif

