#include <stdio.h>

#include "logSpeed.h"


int main() {
  logSpeedType l;

  logSpeedInit(&l);

  sleep(1);
  logSpeedAdd(&l, 1000);
  sleep(1);
  logSpeedAdd(&l, 500);
  sleep(1);
  logSpeedAdd(&l, 10000);
  sleep(1);
  logSpeedAdd(&l, 95000);
  sleep(1);
  logSpeedAdd(&l, 20000);

  fprintf(stderr,"%f\n", logSpeedMedian(&l));

  logSpeedFree(&l);

  return 0;
}



	  
