
#include <malloc.h>
#include <assert.h>

#include "ipcheck.h"

ipCheckType *ipCheckInit() {
  ipCheckType *p = calloc(1, sizeof(ipCheckType));
  return p;
}
			  
  
void ipCheckAdd(ipCheckType *i, unsigned int low, unsigned int high) {

  size_t startcheck = i->num;
  
  i->num += (high - low + 1);
  i->ip= realloc(i->ip, i->num * sizeof(unsigned int)); assert(i->ip);
  i->checked = realloc(i->checked, i->num * sizeof(unsigned char)); assert(i->checked);
  for (size_t j = low; j <= high; j++) {
    i->ip[startcheck] = j;
    i->checked[startcheck] = 0;
    startcheck++;
  }
}


void ipCheckFree(ipCheckType *i) {
  free(i->ip);
  free(i->checked);
  free(i);
}
