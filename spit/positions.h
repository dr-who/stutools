#ifndef _POSITIONS_H
#define _POSITIONS_H

#include <stdio.h>

#include "devices.h"
#include "diskStats.h"
#include "jobType.h"
#include "lengths.h"

typedef struct {
  size_t pos;                    // 8
  double submitTime, finishTime; // 16
  unsigned int len;              // 4;
  unsigned int verify;                // pointer to the value to verify
  unsigned short deviceid;           // 2
  unsigned short seed;           // 2
  float *latencies;
  float sumLatency;
  float usoffset;
  unsigned short samples;
  unsigned short q;              // 2
  char  action;                  // 1: 'R' or 'W'
  unsigned short  success:2;               // 0.5
  unsigned short inFlight:2;
} positionType;

typedef struct {
  positionType *positions;
  size_t sz;
  double LBAcovered;
  //  char *string;
  //  char *device;
  size_t maxbdSize;
  size_t minbdSize;
  size_t minbs;
  size_t maxbs;
  size_t writtenBytes;
  size_t writtenIOs;
  size_t readBytes;
  size_t readIOs;
  size_t UUID;
  double elapsedTime;
  size_t inFlight;
  diskStatType *diskStats;
} positionContainer;

//positionType *createPositions(size_t num);
int poscompare(const void *p1, const void *p2);

int positionContainerCheck(const positionContainer *pc, const size_t minmaxbdSizeBytes, const size_t maxbdSizeBytes, size_t exitonerror);
void positionContainerSave(const positionContainer *p, FILE *name, const size_t bdSizeBytes, const size_t flushEvery, jobType *job);

void positionDumpOne(FILE *fp, const positionType *p, const size_t maxbdSizeBytes, const size_t doflush, const char *name, const size_t index);

//positionType *loadPositions(FILE *fd, size_t *num, deviceDetails **devs, size_t *numDevs, size_t *maxsize);

//void infoPositions(const deviceDetails *devList, const size_t devCount);
size_t positionContainerCreatePositionsGC(positionContainer *pc,
					const lengthsType *len,
					const size_t minbdSize,
					  const size_t maxbdSize,
					  const size_t gcoverhead) ;

size_t positionContainerCreatePositions(positionContainer *pc,
                                        const unsigned short deviceid,
                                        const float sforig,
                                        const size_t sf_maxsizebytes,
                                        const probType readorwrite,
                                        const lengthsType *len,
                                        //const size_t lowbs,
                                        //					const size_t bs,
                                        const size_t alignment,
                                        const long startingBlock,
                                        const size_t minbdSize,
                                        const size_t bdSizeTotal,
                                        unsigned short seed,
                                        const size_t mod,
                                        const size_t remain,
                                        const double fourkEveryMiB,
                                        const size_t jumpKiB,
                                        const size_t firstPPositions,
					const size_t randomSubSample,
					const size_t linearSubSample,
					const size_t linearAlternate,
					const size_t copyMode
                                       );

//void freePositions(positionType *p);
void positionStats(const positionType *positions, const size_t num, const deviceDetails *devList, const size_t devCount);

void positionContainerInit(positionContainer *pc, size_t UUID);
void positionContainerSetup(positionContainer *pc, size_t sz);
void positionContainerSetupFromPC(positionContainer *pc, const positionContainer *oldpc);
void positionContainerFree(positionContainer *pc);

jobType positionContainerLoad(positionContainer *pc, FILE *fd);

jobType positionContainerLoadLines(positionContainer *pc, FILE *fd, const size_t linesMax);

size_t positionContainerAddLinesFilename(positionContainer *pc, jobType *job, const char *filename);
size_t positionContainerAddLines(positionContainer *pc, jobType *job, FILE *fd, const size_t maxLines);

void positionContainerInfo(const positionContainer *pc);
void positionLatencyStats(positionContainer *pc, const int threadid);
void positionContainerRandomize(positionContainer *pc, unsigned int seed);
void positionContainerAddMetadataChecks(positionContainer *pc, const size_t metadata);
void positionContainerAddDelay(positionContainer *pc, double iops, size_t threadid, const double redsec);

void positionContainerInfo(const positionContainer *pc);

positionContainer positionContainerMerge(positionContainer *p, const size_t numFiles);

void positionContainerCollapse(positionContainer *merged);

positionContainer positionContainerMultiply(const positionContainer *original, const size_t multiply);
void positionContainerJumble(positionContainer *pc, const size_t jumble, unsigned int seed);

void calcLBA(positionContainer *pc);
void positionAddBlockSize(positionType *positions, const size_t count, const size_t addSize, const size_t minbdSize, const size_t maxbdSize);

void positionPrintMinMax(positionType *positions, const size_t count, const size_t minbdsize, const size_t maxbdsize, const size_t glow, const size_t ghigh);

void positionContainerDump(positionContainer *pc, const size_t countToShow);
void positionContainerCheckOverlap(const positionContainer *merged);
void positionContainerUniqueSeeds(positionContainer *pc, unsigned short seed, const int andVerify);
void insertFourkEveryMiB(positionContainer *pc, const size_t minbdSize, const size_t maxbdSize, unsigned int seed, const double fourkEveryMiB, const size_t jumpKiB);

void positionContainerHTML(positionContainer *p, const char *name);
void positionContainerModOnly(positionContainer *pc, const size_t jmod, const size_t threadid);

void positionContainerRandomizeProbandRange(positionContainer *pc, unsigned int seed, const double inprob, const size_t inrange);
void monotonicCheck(positionContainer *pc);

positionType * readPos3Cols(FILE *fp, size_t *sz, size_t *minlen, size_t *maxlen);

void positionsRandomize(positionType *positions, const size_t count, unsigned int seed);

void positionsSortPositions(positionType *positions, const size_t count);
void positionContainerInfo(const positionContainer *pc);


#endif

