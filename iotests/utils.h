#ifndef _UTILS_H
#define _UTILS_H

#include "logSpeed.h"

double timedouble();

void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery);
void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery);

size_t blockDeviceSize(char *fd);
int isBlockDevice(char *name);


#endif


