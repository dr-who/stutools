#ifndef _AIOREADS_H
#define _AIOREADS_H

#include "logSpeed.h"

typedef struct {
  size_t pos;
  char   action; // 'R' or 'W'
  size_t len;
  int    success;
} positionType;

double aioMultiplePositions(const int fd,
			     positionType *positions,
			     const size_t sz,
			     const size_t BLKSIZE,
			     const float secTimeout,
			     const size_t QD,
			     const int verbose,
			     const int tableMode,
			     logSpeedType *l,
			     const char *randomBuffer);

int aioVerifyWrites(const char *path,
		    const positionType *positions,
		    const size_t maxpos,
		    const size_t maxBufferSize,
		    const int verbose,
		    const char *randomBuffer);


#endif
