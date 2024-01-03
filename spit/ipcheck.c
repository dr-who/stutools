
#include <malloc.h>
#include <assert.h>

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
  i->ip= realloc(i->ip, i->num * sizeof(unsigned int)); assert(i->ip);
  i->checked = realloc(i->checked, i->num * sizeof(unsigned char)); assert(i->checked);
  i->interface = realloc(i->interface, i->num * sizeof(char*)); assert(i->interface);
  for (size_t j = low; j <= high; j++) {
    i->ip[startcheck] = j;
    i->checked[startcheck] = 0;
    i->interface[startcheck] = interface;
    startcheck++;
  }
}



void ipCheckOpenPort(ipCheckType *ips, size_t port) {
  for (size_t i = 0; i < ips->num; i++) {
    unsigned int ip = ips->ip[i];
    unsigned int ip1, ip2, ip3, ip4;
    ipRangeNtoA(ip, &ip1, &ip2, &ip3, &ip4);
    char s[20];
    sprintf(s,"%u.%u.%u.%u", ip1, ip2, ip3, ip4);
    int fd = sockconnect(s, port);
    if (fd >= 0) {
      printf("found open port %zd on server %s\n", port, s);
    } else {
      printf("nope %s\n", s);
    }
      
  }
}

void ipCheckFree(ipCheckType *i) {
  free(i->ip);
  free(i->checked);
  free(i);
}
