#include <stdio.h>
#include <stdlib.h>

#include "interfaces.h"

int keepRunning = 1;

// ./network  | jq '.[] | select(.device == "eth0")'

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  interfacesIntType *n = interfacesInit();

  interfacesScan(n);
  char *hwserial = interfacesOnboardMac(n);
  if (hwserial) {
    printf("%s\n", hwserial);
  } else {
    printf("n/a (problem)\n");
  }
  free(hwserial);
  
  interfacesFree(n);

  return 0;
}


