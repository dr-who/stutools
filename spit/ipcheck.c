
#include <malloc.h>
#include <assert.h>
#include <stdlib.h>


#include "ipcheck.h"

#include "iprange.h"
#include "simpmail.h"

ipCheckType *ipCheckInit() {
  ipCheckType *p = calloc(1, sizeof(ipCheckType));
  return p;
}
			  
  
void ipCheckAdd(ipCheckType *i, char *interface, unsigned int low, unsigned int high) {

  size_t startcheck = i->num;
  
  i->num += (high - low + 1);
  i->ips= realloc(i->ips, i->num * sizeof(ipType)); assert(i->ips);
  i->interface = realloc(i->interface, i->num * sizeof(char*)); assert(i->interface);
  unsigned int count = 0;
  for (unsigned int j = low; j <= high; j++) {
    i->ips[startcheck].ip = j;
    i->ips[startcheck].bat= count++;
    i->interface[startcheck] = interface;
    startcheck++;
  }
  i->batch++;
}

static int  cmpstringp(const void *p1, const void *p2) {
  ipType *co1=(ipType*)p1, *co2=(ipType*)p2;

  return co1->bat - co2->bat;
}


void ipCheckOpenPort(ipCheckType *ips, size_t port, const double timeout) {
  qsort(ips->ips, ips->num, sizeof(ipType), cmpstringp);

  
  for (size_t i = 0; i < ips->num; i++) {
    unsigned int ip = ips->ips[i].ip;
    unsigned int ip1, ip2, ip3, ip4;
    ipRangeNtoA(ip, &ip1, &ip2, &ip3, &ip4);
    char s[20];
    sprintf(s,"%u.%u.%u.%u", ip1, ip2, ip3, ip4);
    int fd = sockconnect(s, port, timeout);
    if (fd >= 0) {
      printf("found open port %zd on server %s\n", port, s);
    } else {
      printf("nope %s\n", s);
    }
      
  }
}

void ipCheckFree(ipCheckType *i) {
  free(i->ips);
  free(i);
}
