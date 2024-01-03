
#include "ipcheck.h"
#include "iprange.h"

int main() {
  ipRangeType *r = ipRangeInit("172.17.0.0/24");
  ipCheckType *ipc = ipCheckInit();
  ipCheckAdd(ipc, "eth0", r->firstIP, r->lastIP);

  ipCheckOpenPort(ipc, 1600);

  return 0;
}
		  
