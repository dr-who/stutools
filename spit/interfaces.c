#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <memory.h>
#include <arpa/inet.h>
#include <time.h>
#include <math.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <assert.h>
#include <sys/ioctl.h>
#include <net/if.h>
#include <netdb.h>
#include <ifaddrs.h>
#include <signal.h>

#include "interfaces.h"
#include "utils.h"

#include "iprange.h"


char *getHWAddr(const char *nic) {

  char buf[100];
  char *head = buf;
  memset(buf, 0, 100);
  
    int s;
    struct ifreq buffer;

    s = socket(PF_INET, SOCK_DGRAM, 0);
    memset(&buffer, 0x00, sizeof(buffer));
    strcpy(buffer.ifr_name, nic);
    ioctl(s, SIOCGIFHWADDR, &buffer);
    close(s);

    for (s = 0; s < 6; s++) {
      if (s > 0) head += sprintf(head, ":");
      head += sprintf(head, "%.2x", (unsigned char) buffer.ifr_hwaddr.sa_data[s]);
    }
    return strdup(buf);
}


interfacesIntType * interfacesInit() {
  interfacesIntType *n = calloc(1, sizeof(interfacesIntType)); assert(n);
  return n;
}

size_t interfaceSpeed(const char *nic) {
  char ss[PATH_MAX];
  sprintf(ss, "/sys/class/net/%s/speed", nic);  
  size_t speed = getValueFromFile(ss, 1);
  return speed;
}

void interfacesAddDevice(interfacesIntType *d, const char *nic) {
  for (size_t i = 0; i < d->id; i++) {
    if(strcmp(nic, d->nics[i]->devicename)==0) {
      // hit
      return;
    }
  }
  // first look
  // then add
  d->id++;
  d->nics = realloc(d->nics, d->id * sizeof(phyType*));

  phyType *p = calloc(1, sizeof(phyType));
  p->devicename = strdup(nic?nic:"");
  p->hw = getHWAddr(nic);
  p->lastUpdate = timeAsDouble();

  char ss[PATH_MAX];
  sprintf(ss, "/sys/class/net/%s/carrier", nic);
  double linkup = getValueFromFile(ss, 1);
  p->link = linkup;

  //  sprintf(ss, "/sys/class/net/%s/speed", nic);  
  //  double speed = getValueFromFile(ss, 1);
  p->speed = interfaceSpeed(nic);
 

  sprintf(ss, "/sys/class/net/%s/mtu", nic);  
  double mtu = getValueFromFile(ss, 1);
  p->mtu = mtu;

  sprintf(ss, "/sys/class/net/%s/carrier_changes", nic);  
  double carrier_changes = getValueFromFile(ss, 1);
  p->carrier_changes = carrier_changes;

  p->num = 0;
  p->addr = NULL;

  d->nics[d->id - 1] = p;

}

void interfacesAddIP(interfacesIntType *d, const char *nic, const char *ip, const char *netmask, const char *broadcast, const unsigned int cidrMask) {
  assert(d);
  assert(d->id);
  for (size_t i = 0; i < d->id; i++) {
    if (strcmp(d->nics[i]->devicename, nic) == 0) {
            fprintf(stderr,"call: adding to %s, ip %s\n", nic, ip);
      phyType *p = d->nics[i];
      
      int index = p->num;
      p->num++;
      p->addr = realloc(p->addr, p->num * sizeof(addrType));
     
      p->addr[index].addr = strdup(ip?ip:"");
      p->addr[index].netmask = strdup(netmask?netmask:"");
      p->addr[index].broadcast = strdup(broadcast?broadcast:"");
      p->addr[index].cidrMask = cidrMask;

      d->nics[i]->lastUpdate = timeAsDouble();
    }
  }
}

char * interfacesDumpJSONString(const interfacesIntType *d) {
  char *buf = calloc(1, 1000000); assert(buf);
  char *ret = buf;

  buf += sprintf(buf, "[\n");
  for (size_t i = 0; i < d->id; i++) {
    if (i > 0) {
      buf += sprintf(buf,  ",\n");
    }
    buf += sprintf(buf,  "\t{\n");
    phyType *p = d->nics[i];
    buf += sprintf(buf, "\t\t\"device\": \"%s\",\n", p->devicename);
    buf += sprintf(buf, "\t\t\"lastUpdate\": \"%lf\",\n", p->lastUpdate);
    buf += sprintf(buf, "\t\t\"hw\": \"%s\",\n", p->hw);
    buf += sprintf(buf, "\t\t\"link\": %d,\n", p->link);
    buf += sprintf(buf, "\t\t\"speed\": %d,\n", p->speed);
    buf += sprintf(buf, "\t\t\"mtu\": %d,\n", p->mtu);
    buf += sprintf(buf, "\t\t\"carrier_changes\": %d,\n", p->carrier_changes);
    buf += sprintf(buf, "\t\t\"num\": %zd,\n", p->num);
    buf += sprintf(buf, "\t\t\"addresses\": [\n");
    for (size_t j = 0; j < p->num; j++) {
      if (j>0) {
	buf += sprintf(buf, "\t\t\t,\n");
      }
      buf += sprintf(buf, "\t\t\t{\n");
      buf += sprintf(buf, "\t\t\t   \"address\": \"%s\",\n", p->addr[j].addr);
      buf += sprintf(buf, "\t\t\t   \"netmask\": \"%s\",\n", p->addr[j].netmask);
      buf += sprintf(buf, "\t\t\t   \"broadcast\": \"%s\",\n", p->addr[j].broadcast);
      buf += sprintf(buf, "\t\t\t   \"cidrMask\": \"%d\"\n,", p->addr[j].cidrMask);
      char s[100];
      sprintf(s, "%s/%d", p->addr[j].broadcast, p->addr[j].cidrMask);

      
      ipRangeType *ipr = ipRangeInit(s);
      unsigned int ip1, ip2, ip3, ip4;
      int gap = 1;
      if (ipr->lastIP - ipr->firstIP < 3) gap = 0;
      
      ipRangeNtoA(ipr->firstIP+gap, &ip1, &ip2, &ip3, &ip4);
      buf += sprintf(buf, "\t\t\t   \"addressFirst\": \"%u.%u.%u.%u\",\n", ip1, ip2, ip3, ip4);
      ipRangeNtoA(ipr->lastIP-gap, &ip1, &ip2, &ip3, &ip4);
      buf += sprintf(buf, "\t\t\t   \"addressLast\": \"%u.%u.%u.%u\"\n", ip1, ip2, ip3, ip4);
      ipRangeFree(ipr);
      buf += sprintf(buf, "\t\t\t}\n");
    }
    buf += sprintf(buf, "\t\t]\n");
    buf += sprintf(buf, "\t}\n");
  }
  buf += sprintf(buf, "]\n");

  char *retv = strdup(ret);
  free(ret);
  return retv;
  
}


void interfacesDumpJSON(FILE *fd, const interfacesIntType *d) {
  char *s = interfacesDumpJSONString(d);
  fprintf(fd, "%s", s);
  free(s);
}



unsigned int cidrMask(unsigned int n) {
    unsigned int count = 0;
    while (n) {
        count += n & 1;
        n >>= 1;
    }
    return count;
}



void interfacesScan(interfacesIntType *n) {

  assert(n);
  
     struct ifaddrs *ifaddr;
    int family;
    char host[NI_MAXHOST];

    if (getifaddrs(&ifaddr) == -1) {
        perror("getifaddrs");
        exit(EXIT_FAILURE);
    }

    for (struct ifaddrs *ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
        if (ifa->ifa_addr == NULL)
            continue;

        family = ifa->ifa_addr->sa_family;

        /* Display interface name and family (including symbolic
           form of the latter for the common families). */

        if (family == AF_INET/* || family == AF_INET6*/) {
	  //	  if (strcmp(ifa->ifa_name, "lo")==0) {
	  //	    continue; // don't print info on localhost
	  //	  }
	  
	  //	  printf("%s\t", ifa->ifa_name);

	  interfacesAddDevice(n, ifa->ifa_name);

	  int s = getnameinfo(ifa->ifa_addr,
                            (family == AF_INET) ? sizeof(struct sockaddr_in) :
                            sizeof(struct sockaddr_in6),
                            host, NI_MAXHOST,
                            NULL, 0, NI_NUMERICHOST);
            if (s != 0) {
                printf("getnameinfo() failed: %s\n", gai_strerror(s));
                exit(EXIT_FAILURE);
            }


	    char addressBuffer[INET_ADDRSTRLEN];
	    char maskBuffer[INET_ADDRSTRLEN];
	    char broadcastBuffer[INET_ADDRSTRLEN];
	    
	    void *tmpAddrPtr = &((struct sockaddr_in *)(ifa->ifa_addr))->sin_addr;
	    inet_ntop(AF_INET, tmpAddrPtr, addressBuffer, INET_ADDRSTRLEN);
	    
	    tmpAddrPtr = &((struct sockaddr_in *)(ifa->ifa_netmask))->sin_addr;
	    inet_ntop(AF_INET, tmpAddrPtr, maskBuffer, INET_ADDRSTRLEN);
	    

	    tmpAddrPtr = &((struct sockaddr_in *)(ifa->ifa_broadaddr))->sin_addr;
	    inet_ntop(AF_INET, tmpAddrPtr, broadcastBuffer, INET_ADDRSTRLEN);
	    
	    unsigned int tmpMask = ((struct sockaddr_in *)(ifa->ifa_netmask))->sin_addr.s_addr;
	    
	    //	    	    printf("%s IP Address %s, Mask %s, %u\n", ifa->ifa_name, addressBuffer, maskBuffer, cidrMask(tmpMask));
	    //	    	    printf("tmpmask=%u\n", tmpMask);
	    //	                printf("%s\t", host);

	    interfacesAddIP(n, ifa->ifa_name, host, maskBuffer, broadcastBuffer, cidrMask(tmpMask));
        }
    }

    freeifaddrs(ifaddr);
  
}


void interfacesFree(interfacesIntType *n) {
  for (size_t i = 0; i < n->id; i++) {
    phyType *p = n->nics[i];
    for (size_t j = 0; j < p->num; j++) {
      free(p->addr->addr); p->addr->addr = NULL;
      free(p->addr->netmask); p->addr->netmask = NULL;
      free(p->addr->broadcast); p->addr->broadcast = NULL;
    }
    free(p->addr); p->addr = NULL;
    free(p->devicename); p->devicename = NULL;
    free(p->hw); p->hw = NULL;
    free(p); p = NULL;
  }
  free(n->nics); n->nics = NULL;
  free(n);
}  
