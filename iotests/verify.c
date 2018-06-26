#define _POSIX_C_SOURCE 200809L


/**
 * verify.c
 *
 * the main() for ./verify, a standalone position verifier
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "devices.h"
#include "utils.h"
#include "blockVerify.h"
  
int verbose = 1;
int keepRunning = 1;
    
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

  // find block and seed
  long seed;
  size_t blocksize;
  findSeedMaxBlock(positions, numPositions, &seed, &blocksize);

  // generate random buffer with the right seed, do this linearly to avoid random numbers running in parallel
  char *randomBuffer = NULL;
  CALLOC(randomBuffer, blocksize, 1);

  generateRandomBuffer(randomBuffer, blocksize, seed);
  
  // after loading in the positions with the paths, open the files and populate BD sizes etc
  openDevices(devices, numDevices, 0, 0, blocksize, blocksize, blocksize, 0, 0);
  // display
  infoDevices(devices, numDevices);

  fprintf(stderr,"*info* numPositions %zd\n", numPositions);
  // save to do a rough check
  //  savePositions("test.log", positions, numPositions, 0);

  size_t correct = 0, incorrect = 0, ioerrors = 0, lenerrors = 0;
  if (positions) {
    // 256 threads
    verifyPositions(positions, numPositions, randomBuffer, 256, seed, blocksize, &correct, &incorrect, &ioerrors, &lenerrors);
  }
  
  fprintf(stderr,"*info* total %zd, correct %zd, incorrect %zd, ioerrors %zd, lenerrors %zd\n", correct+incorrect+ioerrors+lenerrors, correct, incorrect, ioerrors, lenerrors);

  freeDeviceDetails(devices, numDevices);
  free(positions);
  free(randomBuffer);

  size_t errors = incorrect + ioerrors + lenerrors;
  if (errors > 254) {
    errors = 254; // exit codes have a limit
  }
  exit(errors);
}
  
  
