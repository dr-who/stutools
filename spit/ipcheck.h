#ifndef __IPCHECK_H
#define __IPCHECK_H

#include <stddef.h>

typedef struct {
  unsigned int ip;
  unsigned short bat;
  unsigned char found;
  unsigned char checked;
} ipType;

typedef struct {
  size_t num;
  ipType *ips;
  size_t batch;
  //  char **interface;
} ipCheckType;


ipCheckType *ipCheckInit();

void ipCheckAdd(ipCheckType *i, char *eth, unsigned int low, unsigned int high);

unsigned int ipCheckOpenPort(ipCheckType *ips, size_t port, const double timeout, const int quiet);

void ipCheckFree(ipCheckType *i);

void ipCheckShowFound(ipCheckType *ips);

void ipCheckAllInterfaceRanges(ipCheckType *ipc);


#endif
  

