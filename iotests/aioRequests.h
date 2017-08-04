#ifndef _AIOREADS_H
#define _AIOREADS_H

#include "logSpeed.h"

double aioMultiplePositions(const int fd,
			     const size_t *positions,
			     const size_t sz,
			     const size_t BLKSIZE,
			     const float secTimeout,
			     const size_t QD,
			     const double readRatio,
			     const int verbose,
			     const int tableMode,
			     logSpeedType *l);

#endif
