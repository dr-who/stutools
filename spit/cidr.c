
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>

#include "iprange.h"
#include "ipcheck.h"

int keepRunning = 1;

void usage(void) {
  printf("usage: ./cidr <cidr> ... <cidr>\n\n");
  printf("examples:\n");
  printf("  192.168.5.0/24  # ...\n");
  printf("  192.168.1.128/25  # ...\n");
  printf("  10.0.201.0/24 10.0.220.0/24  # ...\n");
}

int main(int argc, char *argv[]) {
  if (argc < 2) {
    usage();
  } else {
    ipCheckType *ipcheck = ipCheckInit();
    
    for (int v = 1; v < argc; v++) {
      ipRangeType *i = ipRangeInit(argv[v]);
      if (i) {

	ipCheckAdd(ipcheck, "", i->firstIP+1, i->lastIP-1); // remove network and broadcast
	
	fprintf(stderr, "cidr:  %s\n", argv[v]);
	
	unsigned int ip1 = 0, ip2 = 0, ip3 = 0, ip4 = 0;
	
	ipRangeNtoA(i->firstIP, &ip1, &ip2, &ip3, &ip4);
	fprintf(stderr, "address start: %d.%d.%d.%d (network address)\n", ip1, ip2, ip3, ip4);
	
	ipRangeNtoA(i->lastIP, &ip1, &ip2, &ip3, &ip4);
	fprintf(stderr, "address end:   %d.%d.%d.%d (broadcast address)\n", ip1, ip2, ip3, ip4);
	
	// the first and last address in a /30 and larger are the broadcast addresses
	int gap = 1;
	if (i->lastIP - i->firstIP <=2 ) {
	  gap = 0;
	}
	for (unsigned int ii = i->firstIP +gap; ii <= i->lastIP-gap; ii++) {
	  ipRangeNtoA(ii, &ip1, &ip2, &ip3, &ip4);
	  printf("%d.%d.%d.%d\n", ip1, ip2, ip3, ip4);
	}
      } else {
	printf("error: %s isn't a valid CIDR\n", argv[v]);
      }
      ipRangeFree(i);
    }
    fprintf(stderr,"tocheck: %zd\n", ipcheck->num);
    ipCheckFree(ipcheck);
  }


  return 0;
}
