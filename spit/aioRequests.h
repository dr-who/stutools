#ifndef _AIOREADS_H
#define _AIOREADS_H

#include <libaio.h>

#include "logSpeed.h"
#include "positions.h"

size_t aioMultiplePositions( positionContainer *p,
                             const size_t sz,
                             const double finishtime,
                             const size_t finishBytes,
                             size_t QD,
                             const int verbose,
                             const int tableMode,
                             const size_t alignment,
                             size_t *ios,
                             size_t *totalRB,
                             size_t *totalWB,
                             const size_t oneShot,
                             const int dontExitOnErrors,
                             const int fd,
                             int flushEvery,
                             size_t *ioerrors,
                             size_t QDbarrier,
                             const size_t discard_max_bytes,
			     FILE *fp,
			     char *jobdevice
                           );

int aioVerifyWrites(positionType *positions,
                    const size_t maxpos,
                    const size_t maxBufferSize,
                    const size_t alignment,
                    const int verbose,
                    const char *randomBuffer,
                    const int fd);

#endif
