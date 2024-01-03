
#include "ipcheck.h"
#include "iprange.h"

int main() {
  ipRangeType *r = ipRangeInit("192.168.9.255/24");
  ipCheckType *ipc = ipCheckInit();
  ipCheckAdd(ipc, "eth0", r->firstIP, r->lastIP);

  ipCheckOpenPort(ipc, 1600);

  return 0;
}
		  
