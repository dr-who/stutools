#ifndef __NETWORK_H
#define __NETWORK_H

#include <stdio.h>

typedef struct {
  char *addr;
  char *netmask;
  unsigned int cidrMask;
} addrType;

typedef struct {
  size_t id;
  char **devicename;
  char **hw;
  double *lastUpdate;
  int *link;
  int *mtu;
  int *speed;
  int *carrier_changes;
  size_t *num;
  addrType **addr;
} networkIntType;


char *getHWAddr(const char *nic);


networkIntType *networkInit();

void networkAddDevice(networkIntType *d, const char *nic);
void networkAddIP(networkIntType *d, const char *nic, const char *ip, const char *netmask, const unsigned int cidrMask);

void networkScan(networkIntType *n);

void networkDumpJSON(FILE *fd, const networkIntType *d);

void networkFree(networkIntType *n);

#endif

