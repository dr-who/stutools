#ifndef __INTERFACES_H
#define __INTERFACES_H

#include <stdio.h>

typedef struct {
  char *addr;
  char *netmask;
  char *broadcast;
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
} interfacesIntType;


char *getHWAddr(const char *nic);


interfacesIntType *interfacesInit();

void interfacesAddDevice(interfacesIntType *d, const char *nic);
void interfacesAddIP(interfacesIntType *d, const char *nic, const char *ip, const char *netmask, const char *broadcast, const unsigned int cidrMask);

void interfacesScan(interfacesIntType *n);

size_t interfaceSpeed(const char *nic);

char * interfacesDumpJSONString(const interfacesIntType *d);
void interfacesDumpJSON(FILE *fd, const interfacesIntType *d);

void interfacesFree(interfacesIntType *n);

#endif

