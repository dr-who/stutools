#include <string.h>

#include "interfaces.h"
#include "ipcheck.h"
#include "iprange.h"

int keepRunning = 1;

int main() {
  interfacesIntType *n = interfacesInit();

  interfacesScan(n);

  ipCheckType *ipc = ipCheckInit();

  for (size_t i = 0; i < n->id; i++) {
    phyType *p = n->nics[i];
    if (strcmp(p->devicename, "lo") != 0) {
      printf("%s\n", p->devicename);
      for (size_t j = 0; j < p->num; j++) {
	addrType *a = &p->addr[j];
	ipRangeType *r = ipRangeInit2(a->broadcast, a->cidrMask);
  
	//  ipRangeType *r = ipRangeInit("192.168.9.255/24");
	ipCheckAdd(ipc, p->devicename, r->firstIP, r->lastIP);
      }
    }
  }

  ipCheckOpenPort(ipc, 1600, 0.1);

  ipShowFound(ipc);
  
  return 0;
}
		  
