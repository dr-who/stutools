#include <stddef.h>
#include "interfaces.h"

int keepRunning = 1;

// ./network  | jq '.[] | select(.device == "eth0")'

int main() {
  interfacesIntType *n = interfacesInit();

  interfacesScan(n);
  interfacesDumpJSON(stdout, n);
  


  interfacesFree(n);

  return 0;
}


