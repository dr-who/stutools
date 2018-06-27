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
  
int verbose = 2;
int keepRunning = 1;
int flushEvery = 0;
    
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
  positions = loadPositions(stdin, &numPositions, &devices, &numDevices);
  if (!positions) {
    fprintf(stderr,"*warning* no valid positions\n");
    freeDeviceDetails(devices, numDevices);
    exit(-1);
  }

  // find block and seed
  long seed;
  size_t blocksize;
  findSeedMaxBlock(positions, numPositions, &seed, &blocksize);

  // generate random buffer with the right seed, do this linearly to avoid random numbers running in parallel
  char *randomBuffer = NULL;
  CALLOC(randomBuffer, blocksize, 1);

  generateRandomBuffer(randomBuffer, blocksize, seed);
  
  // after loading in the positions with the paths, open the files and populate BD sizes etc
  openDevices(devices, numDevices, 0, 0, blocksize, blocksize, blocksize, 1, 0); // the 1 is "need to write"
  // display
  infoDevices(devices, numDevices);
  int numOpen = numOpenDevices(devices, numDevices);

  if (numOpen <= 0) {
    fprintf(stderr,"*error* no devices can be opened\n");
    exit(-1);
  }

  fprintf(stderr,"*info* numPositions %zd\n", numPositions);

  size_t rb = 0, ios = 0, totalRB = 0, totalWB = 0;
  double start = timedouble();
  if (positions) {
    logSpeedType l;
    logSpeedInit(&l);
    rb = aioMultiplePositions(positions, numPositions, 100000, 256, 1, 0, &l, NULL, randomBuffer, blocksize, blocksize, &ios, &totalRB, &totalWB, 1, 0);
    logSpeedFree(&l);
  }
  double elapsed = timedouble() - start;
  fprintf(stderr,"*info* %zd bytes (%.3lf GiB), read %.3lf GiB, write %.3lf GiB in %.1lf seconds. Total speed %.1lf MiB/sec\n", rb, TOGiB(rb), TOGiB(totalRB), TOGiB(totalWB), elapsed, TOMiB(rb) / elapsed);
  
  freeDeviceDetails(devices, numDevices);
  free(positions);
  free(randomBuffer);

  exit(0);
}
  
  
