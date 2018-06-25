#include <stdlib.h>


#include "positions.h"
#include "devices.h"

int verbose = 0;
int keepRunning = 1;


int main(int argc, char *argv[]) {

  deviceDetails *devs = NULL;
  size_t numDevs = 0;
  positionType *positions = NULL;
  size_t num = 0;
  
  positions = loadPositions(stdin, &num, &devs, &numDevs);

  openDevices(devs, numDevs, 0, 0, 0, 0, 0, 0, 0);
  infoDevices(devs, numDevs);

  fprintf(stderr,"num %zd\n", num);
  savePositions("test.log", devs, numDevs, positions, num, 0, 0, 0);

  
  if (positions) {
  }

  freeDeviceDetails(devs, numDevs);

}
  
  
