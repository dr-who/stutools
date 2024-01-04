#include <string.h>
#include <malloc.h>

#include "interfaces.h"
#include "ipcheck.h"
#include "iprange.h"

int keepRunning = 1;

int main() {
  ipCheckType *ipc = ipCheckInit();
  ipCheckAllInterfaceRanges(ipc, 1600, 0.1, 0);
  ipCheckShowFound(ipc);
  ipCheckFree(ipc);

  return 0;
}
		  
