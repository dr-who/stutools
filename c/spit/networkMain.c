#include <stddef.h>
#include "network.h"

int keepRunning = 1;

// ./network  | jq '.[] | select(.device == "eth0")'

int main() {
  networkIntType *n = networkInit();

  interfaceScan(n);
  interfaceDumpJSON(stdout, n);
  


  networkFree(n);

  return 0;
}


