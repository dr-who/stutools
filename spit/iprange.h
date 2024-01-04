#ifndef __IPRANGE_H
#define __IPRANGE_H


typedef struct {
  char *cidr;
  unsigned int firstIP;
  unsigned int lastIP;
  size_t toCheck;
} ipRangeType;



ipRangeType *ipRangeInit(const char *cidr);
ipRangeType *ipRangeInit2(const char *broadcast, const size_t cidrMask);


void ipRangeNtoA(unsigned int n, unsigned int *ip1, unsigned int *ip2, unsigned int *ip3, unsigned int *ip4);

void ipRangeFree(ipRangeType *i);

#endif
  

