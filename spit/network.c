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

#include "network.h"
#include "utils.h"


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


networkIntType * networkInit() {
  networkIntType *n = calloc(1, sizeof(networkIntType)); assert(n);
  return n;
}


void networkAddDevice(networkIntType *d, const char *nic) {
  // first look
  // then add
  d->id++;
  d->devicename = realloc(d->devicename, d->id * sizeof(char*));
  d->devicename[d->id-1] = strdup(nic?nic:"");
  
  d->num = realloc(d->num, d->id * sizeof(size_t));
  d->num[d->id-1] = 0; // if added there are no known IPs

  d->lastUpdate = realloc(d->lastUpdate, d->id * sizeof(double));
  d->lastUpdate[d->id-1] = timeAsDouble();

  d->hw = realloc(d->hw, d->id * sizeof(char*));
  d->hw[d->id-1] = getHWAddr(nic);

  char ss[PATH_MAX];
  sprintf(ss, "/sys/class/net/%s/carrier", nic);
  double linkup = getValueFromFile(ss, 1);
  d->link = realloc(d->link, d->id * sizeof(int*));
  d->link[d->id - 1] = linkup;

  sprintf(ss, "/sys/class/net/%s/speed", nic);  
  double speed = getValueFromFile(ss, 1);
  d->speed = realloc(d->speed, d->id * sizeof(int*));
  d->speed[d->id - 1] = speed;
 

    sprintf(ss, "/sys/class/net/%s/mtu", nic);  
  double mtu = getValueFromFile(ss, 1);
  d->mtu = realloc(d->mtu, d->id * sizeof(int*));
  d->mtu[d->id - 1] = mtu;

  sprintf(ss, "/sys/class/net/%s/carrier_changes", nic);  
  double carrier_changes = getValueFromFile(ss, 1);
  d->carrier_changes = realloc(d->carrier_changes, d->id * sizeof(int*));
  d->carrier_changes[d->id - 1] = carrier_changes;

}

void networkAddIP(networkIntType *d, const char *nic, const char *ip, const char *netmask, const unsigned int cidrMask) {
  for (size_t i = 0; i < d->id; i++) {
    if (strcmp(d->devicename[i], nic) == 0) {
      d->num[i]++;
      d->addr = realloc(d->addr, d->num[i] * sizeof(addrType*));
      d->addr[d->num[i]-1]= calloc(1, sizeof(addrType));
      d->addr[d->num[i]-1]->addr = strdup(ip?ip:"");
      d->addr[d->num[i]-1]->netmask = strdup(netmask?netmask:"");
      d->addr[d->num[i]-1]->cidrMask = cidrMask;

      d->lastUpdate[i] = timeAsDouble();
    }
  }
}

char * networkDumpJSONString(const networkIntType *d) {
  char *buf = calloc(1, 1000000); assert(buf);
  char *ret = buf;

  buf += sprintf(buf, "[\n");
  for (size_t i = 0; i < d->id; i++) {
    if (i > 0) {
      buf += sprintf(buf,  ",\n");
    }
    buf += sprintf(buf,  "\t{\n");
    buf += sprintf(buf, "\t\t\"device\": \"%s\",\n", d->devicename[i]);
    buf += sprintf(buf, "\t\t\"lastUpdate\": \"%lf\",\n", d->lastUpdate[i]);
    buf += sprintf(buf, "\t\t\"hw\": \"%s\",\n", d->hw[i]);
    buf += sprintf(buf, "\t\t\"link\": %d,\n", d->link[i]);
    buf += sprintf(buf, "\t\t\"speed\": %d,\n", d->speed[i]);
    buf += sprintf(buf, "\t\t\"mtu\": %d,\n", d->mtu[i]);
    buf += sprintf(buf, "\t\t\"carrier_changes\": %d,\n", d->carrier_changes[i]);
    buf += sprintf(buf, "\t\t\"num\": %zd,\n", d->num[i]);
    buf += sprintf(buf, "\t\t\"addresses\": [\n");
    for (size_t j = 0; j < d->num[i]; j++) {
      if (j>0) {
	buf += sprintf(buf, "\t\t\t,\n");
      }
      buf += sprintf(buf, "\t\t\t{\n");
      buf += sprintf(buf, "\t\t\t   \"address\": \"%s\",\n", d->addr[j]->addr);
      buf += sprintf(buf, "\t\t\t   \"netmask\": \"%s\",\n", d->addr[j]->netmask);
      buf += sprintf(buf, "\t\t\t   \"cidrMask\": \"%d\"\n", d->addr[j]->cidrMask);
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


void networkDumpJSON(FILE *fd, const networkIntType *d) {
  char *s = networkDumpJSONString(d);
  fprintf(fd, "%s", s);
}



unsigned int cidrMask(unsigned int n) {
    unsigned int count = 0;
    while (n) {
        count += n & 1;
        n >>= 1;
    }
    return count;
}



void networkScan(networkIntType *n) {

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

	  networkAddDevice(n, ifa->ifa_name);

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
	    
	    void *tmpAddrPtr = &((struct sockaddr_in *)(ifa->ifa_addr))->sin_addr;
	    inet_ntop(AF_INET, tmpAddrPtr, addressBuffer, INET_ADDRSTRLEN);
	    
	    tmpAddrPtr = &((struct sockaddr_in *)(ifa->ifa_netmask))->sin_addr;
	    inet_ntop(AF_INET, tmpAddrPtr, maskBuffer, INET_ADDRSTRLEN);
	    
	    unsigned int tmpMask = ((struct sockaddr_in *)(ifa->ifa_netmask))->sin_addr.s_addr;
	    
	    //	    printf("%s IP Address %s, Mask %s, %u\n", ifa->ifa_name, addressBuffer, maskBuffer, cidrMask(tmpMask));
	    //	    printf("tmpmask=%u\n", tmpMask);
	    
	    //            printf("%s\t", host);
	    
	    networkAddIP(n, ifa->ifa_name, host, maskBuffer, cidrMask(tmpMask));
	    //	    networkAddIP(n, ifa->ifa_name, host, maskBuffer, cidrMask(tmpMask));
        }
    }

    freeifaddrs(ifaddr);
  
}


void networkFree(networkIntType *n) {
  return ;
  for (size_t i = 0; i < n->id; i++) {
      addrType **a = &n->addr[i];
      for (size_t j = 0; j < n->num[i]; j++) {
	free(a[j]->addr);
	free(a[j]->netmask);
      }
      free(a);
  }
  
  for (size_t i = 0; i < n->id; i++) {
    free(n->devicename[i]);
    free(n->hw[i]);
  }
  free(n->devicename); 
  free(n->hw); 
  free(n->addr);
  free(n->lastUpdate); 
  free(n->link); 
  free(n->mtu); 
  free(n->speed); 
  free(n->carrier_changes); 
}  
