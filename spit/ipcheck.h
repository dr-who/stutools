#ifndef __IPCHECK_H
#define __IPCHECK_H

#include <stddef.h>

typedef struct {
  size_t num;
  unsigned int *ip;
  unsigned char *checked;
  char **interface;
} ipCheckType;


ipCheckType *ipCheckInit();

void ipCheckAdd(ipCheckType *i, char *eth, unsigned int low, unsigned int high);

void ipCheckOpenPort(ipCheckType *ips, size_t port);

void ipCheckFree(ipCheckType *i);

#endif
  

