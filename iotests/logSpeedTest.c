#include <stdio.h>

#include "logSpeed.h"

int keepRunning = 1;

int main() {
  logSpeedType l;

  logSpeedInit(&l);

  logSpeedAdd(&l, 0.2);
  logSpeedAdd(&l, 0.3);
  logSpeedAdd(&l, 0.2);
  logSpeedAdd(&l, 0.4);
  logSpeedAdd(&l, 10);

  fprintf(stderr,"%f\n", logSpeedMean(&l));

  logSpeedFree(&l);

  return 0;
}



	  
