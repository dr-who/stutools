#ifndef _AIOREADS_H
#define _AIOREADS_H

#include "logSpeed.h"
#include "positions.h"

size_t aioMultiplePositions(const int fd,
			     positionType *positions,
			     const size_t sz,
			     const float secTimeout,
			     const size_t QD,
			     const int verbose,
			     const int tableMode,
			     logSpeedType *l,
			     char *randomBuffer,
			     const size_t randomBufferSize,
			    const size_t alignment,
			    size_t *ios,
			    size_t *totalRB,
			    size_t *totalWB);

int aioVerifyWrites(const char *path,
		    const positionType *positions,
		    const size_t maxpos,
		    const size_t maxBufferSize,
		    const size_t alignment,
		    const int verbose,
		    const char *randomBuffer);

#endif
