#ifndef _POSITIONS_H
#define _POSITIONS_H

#include <stdio.h>

#include "devices.h"

typedef struct {
  size_t pos;                    // 8
  double submittime, finishtime; // 16
  unsigned int len;              // 4;
  unsigned short seed;           // 2
  unsigned short q;              // 2
  char  action;                  // 1: 'R' or 'W'
  unsigned int  success:4;               // 0.5
  unsigned int  verify:4;                // 0.5
} positionType;

typedef struct {
  positionType *positions;
  size_t sz;
  char *string;
  char *device;
  size_t bdSize;
  size_t minbs;
  size_t maxbs;
  size_t writtenBytes;
  size_t writtenIOs;
  size_t readBytes;
  size_t readIOs;
  size_t UUID;
  double elapsedTime;
} positionContainer;

positionType *createPositions(size_t num);

int checkPositionArray(const positionType *positions, size_t num, size_t bdSizeBytes, size_t exitonerror);
void positionContainerSave(const positionContainer *p, const char *name, const size_t bdSizeBytes, const size_t flushEvery);

positionType *loadPositions(FILE *fd, size_t *num, deviceDetails **devs, size_t *numDevs, size_t *maxsize);

void infoPositions(const deviceDetails *devList, const size_t devCount);

size_t setupPositions(positionType *positions,
		    size_t *num,
		    const int sf,
		    const double readorwrite,
		    const size_t lowbs,
		    const size_t bs,
		    size_t alignment,
		    const long startingBlock,
		    const size_t bdSizeTotal,
		    unsigned short seed
		    );

void freePositions(positionType *p);
void positionStats(const positionType *positions, const size_t num, const deviceDetails *devList, const size_t devCount);

void dumpPositions(positionType *positions, const char *prefix, const size_t num, const size_t countToShow);

void positionContainerInit(positionContainer *pc, size_t UUID);
void positionContainerSetup(positionContainer *pc, size_t sz, char *device, char *string);
void positionContainerFree(positionContainer *pc);

void positionContainerLoad(positionContainer *pc, FILE *fp);

void positionContainerInfo(const positionContainer *pc);

void positionLatencyStats(positionContainer *pc, const int threadid);

void positionContainerAddMetadataChecks(positionContainer *pc);

size_t setupRandomPositions(positionType *pos,
			  const size_t num,
			  const double rw,
			  const size_t bs,
			  const size_t highbs,
			  const size_t alignment,
			  const size_t bdSize,
			  const size_t seedin);

size_t numberOfDuplicates(positionType *pos, size_t const num);

void positionContainerInfo(const positionContainer *pc);
void skipPositions(positionType *pos, const size_t num, const size_t skipEvery);

#endif

