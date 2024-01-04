#ifndef __IPCHECK_H
#define __IPCHECK_H

#include <stddef.h>

typedef struct {
  unsigned int ip;
  unsigned short bat;
} ipType;

typedef struct {
  size_t num;
  ipType *ips;
  size_t batch;
  char **interface;
} ipCheckType;


ipCheckType *ipCheckInit();

void ipCheckAdd(ipCheckType *i, char *eth, unsigned int low, unsigned int high);

void ipCheckOpenPort(ipCheckType *ips, size_t port, const double timeout);

void ipCheckFree(ipCheckType *i);

#endif
  

