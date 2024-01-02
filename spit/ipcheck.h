#ifndef __IPCHECK_H
#define __IPCHECK_H


typedef struct {
  size_t num;
  unsigned int *ip;
  unsigned char *checked;
} ipCheckType;


ipCheckType *ipCheckInit();

void ipCheckAdd(ipCheckType *i, unsigned int low, unsigned int high);

void ipCheckFree(ipCheckType *i);

#endif
  

