#include <string.h>
#include <malloc.h>

#include "interfaces.h"
#include "ipcheck.h"
#include "iprange.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  ipCheckType *ipc = ipCheckInit();
  ipCheckAllInterfaceRanges(ipc);

  unsigned int ip = 0;
  while((ip = ipCheckOpenPort(ipc, 1600, 0.1, 1)) != 0) {
    printf("%d\n", ip);
  }


  ipCheckShowFound(ipc);
  ipCheckFree(ipc);

  return 0;
}
		  
