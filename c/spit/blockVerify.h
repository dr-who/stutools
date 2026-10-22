#ifndef _BLOCK_VERIFY_H
#define _BLOCK_VERIFY_H

#include "jobType.h"


int
verifyPosition(const int fd, const positionType *p, const char *randomBuffer, char *buf, size_t *diff, const int seed,
               int quiet, size_t overridesize);

int verifyPositions(positionContainer *pc, const size_t threads, jobType *job, const size_t o_direct, const size_t sort,
                    const double runSeconds, size_t *correct, size_t *incorrect, size_t *ioerrors, int quiet,
                    int process, size_t overridesize, size_t exitafternerrors);

#endif
