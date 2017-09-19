#ifndef _POSITIONS_H
#define _POSITIONS_H

typedef struct {
  int    fd;
  size_t pos;
  char   action; // 'R' or 'W'
  size_t len;
  int    success;
} positionType;

positionType *createPositions(size_t num);

void checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes);
void dumpPositions(positionType *positions, size_t num, size_t bdSizeBytes);

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
		    const size_t startAtZero);



#endif

