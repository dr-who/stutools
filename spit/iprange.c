#include <malloc.h>
#include <assert.h>
#include <stdio.h>

#include "iprange.h"

// cidr_to_ip_and_mask function is from stackoverflow, which is creative commons. https://stackoverflow.com/help/licensing
//
// https://stackoverflow.com/questions/28532688/c-cidr-to-address-list
int cidr_to_ip_and_mask(const char *cidr, unsigned int *ip, unsigned int *mask)
{
   unsigned int a, b, c, d, bits;
    if (sscanf(cidr, "%u.%u.%u.%u/%u", &a, &b, &c, &d, &bits) < 5) {
        return -1; /* didn't convert enough of CIDR */
    }
    if (bits > 32) {
        return -1; /* Invalid bit count */
    }
    *ip =
        (a << 24UL) |
        (b << 16UL) |
        (c << 8UL) |
        (d);
    *mask = (0xFFFFFFFFUL << (32 - bits)) & 0xFFFFFFFFUL;
    return 0;
}

ipRangeType *ipRangeInit(const char *cidr) {
  ipRangeType *p = calloc(1, sizeof(ipRangeType)); assert(p);

  unsigned int mask;

  unsigned int ip;
  if (cidr_to_ip_and_mask(cidr, &ip, &mask) < 0) {
    printf("error");
    return NULL;
  }

  p->firstIP = ip & mask;
  p->lastIP = p->firstIP | ~mask;

  return p;
}

void ipRangeNtoA(unsigned int n, unsigned int *ip1, unsigned int *ip2, unsigned int *ip3, unsigned int *ip4) {
  *ip1 = (n >> 24) & 0xff;
  *ip2 = (n >> 16) & 0xff;
  *ip3 = (n >> 8) & 0xff;
  *ip4 = n & 0xff;


  assert(*ip1 < 256);
  assert(*ip2 < 256);
  assert(*ip3 < 256);
  assert(*ip4 < 256);
}

void ipRangeFree(ipRangeType *i) {
  free(i->cidr);
  free(i);
}
