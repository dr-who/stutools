#include <stddef.h>
#include <assert.h>

#include "interfaces.h"

int keepRunning = 1;

// ./network  | jq '.[] | select(.device == "eth0")'

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  interfacesIntType *n = interfacesInit();

  interfacesScan(n);
  for (size_t i = 0; i < n->id; i++) {
    fprintf(stderr, "%s (speed %d)\n", n->nics[i]->devicename, n->nics[i]->speed);
  }


  
  interfacesDumpJSON(stdout, n);


  fprintf(stderr,"IPs -> %s\n", interfaceIPNonWifi(n));


  interfacesFree(&n); assert(NULL==n);

  return 0;
}


