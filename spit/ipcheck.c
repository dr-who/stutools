
#include <malloc.h>
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>


#include "ipcheck.h"
#include "utilstime.h"
#include "iprange.h"
#include "simpsock.h"

#include "interfaces.h"

ipCheckType *ipCheckInit(void) {
  ipCheckType *p = calloc(1, sizeof(ipCheckType));
  return p;
}
			  
  
void ipCheckAdd(ipCheckType *i, char *interface, unsigned int low, unsigned int high) {
  if (interface) {}

  size_t startcheck = i->num;
  
  i->num += (high - low + 1);
  i->ips= realloc(i->ips, i->num * sizeof(ipType)); assert(i->ips);
  //  i->interface = realloc(i->interface, i->num * sizeof(char*)); assert(i->interface);
  
  unsigned int count = 0;
  for (unsigned int j = low; j <= high; j++) {
    i->ips[startcheck].ip = j;
    i->ips[startcheck].bat= count++;
    i->ips[startcheck].found = 0;
    i->ips[startcheck].checked = 0;
    //    i->interface[startcheck] = strdup(interface);
    startcheck++;
  }
  i->batch++;
}

static int  cmpstringp(const void *p1, const void *p2) {
  ipType *co1=(ipType*)p1, *co2=(ipType*)p2;

  return co1->bat - co2->bat;
}


unsigned int ipCheckOpenPort(ipCheckType *ips, size_t port, const double timeout, const int quiet) {
  qsort(ips->ips, ips->num, sizeof(ipType), cmpstringp);

  size_t limitval = ips->num;
  if (ips->num > 1024) {
    //    fprintf(stderr,"*warning* the IP range/subnet is so large, limiting to 1024 addresses\n");
    limitval = 1024;
  }

  unsigned int ip = 0;

  for (size_t i = 0; i < limitval; i++) if (ips->ips[i].checked == 0) {
    ip = ips->ips[i].ip;
    unsigned int ip1, ip2, ip3, ip4;
    ipRangeNtoA(ip, &ip1, &ip2, &ip3, &ip4);
    char s[20];
    sprintf(s,"%u.%u.%u.%u", ip1, ip2, ip3, ip4);
    if (quiet == 0) printf("[%lf] ", timeAsDouble());
    ips->ips[i].checked = 1;
    int fd = sockconnect(s, port, timeout);
    if (fd >= 0) {
      printf("found open port %zd on server %s\n", port, s);
      ips->ips[i].found = 1;
      shutdown(fd,  SHUT_RDWR);
      close(fd);
      break; // stop after 1
    } else {
      if (quiet == 0) printf("nope %s (%zd of %zd)\n", s, i, limitval-1);
    }
  }
  return ip;
}

void ipCheckShowFound(ipCheckType *ips) {
  for (size_t i = 0; i < ips->num; i++) {
    if (ips->ips[i].found) {
      unsigned int ip = ips->ips[i].ip;
      unsigned int ip1,ip2,ip3,ip4;
      ipRangeNtoA(ip, &ip1, &ip2, &ip3, &ip4);
      char s[20];
      sprintf(s,"%u.%u.%u.%u", ip1, ip2, ip3, ip4);
      printf("%s\n", s);
    }
  }
}

void ipCheckFree(ipCheckType *ipc) {
  for (size_t i =0; i < ipc->num; i++) {
    //    free(ipc->interface[i]);
  }
  //  free(ipc->interface);
  free(ipc->ips);
  free(ipc);
}

void ipCheckAllInterfaceRanges(ipCheckType *ipc) {
  interfacesIntType *n = interfacesInit();

  interfacesScan(n);

  for (size_t i = 0; i < n->id; i++) {
    phyType *p = n->nics[i];
    if (strcmp(p->devicename, "lo") != 0) {
      //printf("%s\n", p->devicename);
      for (size_t j = 0; j < p->num; j++) {
	addrType *a = &p->addr[j];
	fprintf(stderr, "*info* examining %s: %s/%d\n", p->devicename, a->broadcast, a->cidrMask);
	ipRangeType *r = ipRangeInit2(a->broadcast, a->cidrMask);
  
	//  ipRangeType *r = ipRangeInit("192.168.9.255/24");
	ipCheckAdd(ipc, p->devicename, r->firstIP, r->lastIP);
	free(r);
      }
    }
  }
  interfacesFree(n);
}  
