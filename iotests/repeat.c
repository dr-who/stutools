#define _POSIX_C_SOURCE 200809L


/**
 * repeat.c
 *
 * the main() for ./repeat, a standalone position reader/writer
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "devices.h"
#include "utils.h"
#include "aioRequests.h"
  
int verbose = 0;
int keepRunning = 1;
int flushEvery = 0;
size_t waitEvery = 0;
    
/**
 * main
 *
 */
int main(int argc, char *argv[]) {

  deviceDetails *devices = NULL;
  size_t numDevices = 0;
  
  positionType *positions = NULL;
  size_t numPositions = 0;

  // load in all the positions, generation from the -L filename option from aioRWTest
  size_t maxsize = 0;
  positions = loadPositions(stdin, &numPositions, &devices, &numDevices, &maxsize);
  if (!positions) {
    fprintf(stderr,"*warning* no valid positions\n");
    freeDeviceDetails(devices, numDevices);
    exit(-1);
  }
  for (size_t i = 0; i <numDevices; i++) {
    devices[i].shouldBeSize = maxsize;
  }


  // find block and seed
  long seed;
  size_t minbs, blocksize;
  findSeedMaxBlock(positions, numPositions, &seed, &minbs, &blocksize);

  // generate random buffer with the right seed, do this linearly to avoid random numbers running in parallel
  char *randomBuffer = NULL;
  CALLOC(randomBuffer, blocksize, 1);

  generateRandomBuffer(randomBuffer, blocksize, seed);
  
  const size_t qd = 256, contextCount = 1;
  io_context_t *ioc = createContexts(contextCount, qd);
  setupContexts(ioc, contextCount, qd);
  // after loading in the positions with the paths, open the files and populate BD sizes etc
  openDevices(devices, numDevices, 0, &maxsize, minbs, blocksize, minbs, 1, 0, qd, contextCount); // the 1 is "need to write"
  // display
  infoDevices(devices, numDevices);
  int numOpen = numOpenDevices(devices, numDevices);

  if (numOpen <= 0) {
    fprintf(stderr,"*error* no devices can be opened\n");
    exit(-1);
  }

  fprintf(stderr,"*info* numPositions %zd, min block size %zd, block size %zd, alignment %zd\n", numPositions, minbs, blocksize, minbs);

  size_t rb = 0, ios = 0, totalRB = 0, totalWB = 0;
  double start = timeAsDouble();
  if (positions) {
    logSpeedType l;
    logSpeedInit(&l);
    rb = aioMultiplePositions(positions, numPositions, 100000, 256, verbose, 0, &l, NULL, randomBuffer, blocksize, minbs, &ios, &totalRB, &totalWB, 1, 0, ioc, contextCount);
    logSpeedFree(&l);
  }
  double elapsed = timeAsDouble() - start;
  fprintf(stderr,"*info* %zd bytes (%.3lf GiB), read %.3lf GiB, write %.3lf GiB in %.1lf seconds. Total speed %.1lf MiB/sec\n", rb, TOGiB(rb), TOGiB(totalRB), TOGiB(totalWB), elapsed, TOMiB(rb) / elapsed);
  
  freeDeviceDetails(devices, numDevices);
  freeContexts(ioc, contextCount);
  free(positions);
  free(randomBuffer);

  exit(0);
}
  
  
