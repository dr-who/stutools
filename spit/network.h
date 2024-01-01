#ifndef __NETWORK_H
#define __NETWORK_H

#include <stdio.h>

typedef struct {
  char *addr;
  char *netmask;
  unsigned int cidrMask;
} addrType;

typedef struct {
  // physical device (addDevice)
  char *devicename;
  char *hw;
  double lastUpdate;
  int link;
  int mtu;
  int speed;
  int carrier_changes;
  // array of devices
  size_t num;
  addrType *addr;
} phyType;

typedef struct {
  size_t id;
  phyType **nics;
} networkIntType;


char *getHWAddr(const char *nic);


networkIntType *networkInit();

void networkAddDevice(networkIntType *d, const char *nic);
void networkAddIP(networkIntType *d, const char *nic, const char *ip, const char *netmask, const unsigned int cidrMask);

void networkScan(networkIntType *n);

char * networkDumpJSONString(const networkIntType *d);
void networkDumpJSON(FILE *fd, const networkIntType *d);

void networkFree(networkIntType *n);

#endif

