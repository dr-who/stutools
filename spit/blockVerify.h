#ifndef _BLOCK_VERIFY
#define _BLOCK_VERIFY

#include "positions.h"

int verifyPositions(positionType *positions, size_t numPositions,char *randomBuffer, size_t threads, long seed, size_t blocksize, size_t *correct, size_t *incorrect, size_t *ioerrors, size_t *lenerrors);


#endif
