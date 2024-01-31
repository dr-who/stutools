/* 

// base from online

multicast.c

The following program sends or receives multicast packets. If invoked
with one argument, it sends a packet containing the current time to an
1arbitrarily chosen multicast group and UDP port. If invoked with no
arguments, it receives and prints these packets. Start it as a sender on
just one host and as a receiver on all the other hosts

*/

extern int keepRunning;

#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <time.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <stdlib.h>
#include <assert.h>
#include <sys/utsname.h>

#include "utilstime.h"
#include "advertise-mc.h"
#include "multicast.h"
#include "keyvalue.h"
#include "sat.h"
#include "cluster.h"
#include "blockdevices.h"
#include "utils.h"

void *advertiseMC(void *arg) {
  threadMsgType *tc = (threadMsgType *) arg;

    clusterType *cluster = tc->cluster;
  
  int count = 0;
  int addrlen, sock, cnt;
  double starttime = timeAsDouble();
  
  /* set up socket */
  sock = socket(AF_INET, SOCK_DGRAM, 0);
  if (sock < 0) {
    perror("socket");
    exit(1);
  }

  const int enable = 1;
  if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &enable, sizeof(int)) < 0)
    perror("setsockopt(SO_REUSEADDR) failed");
  if (setsockopt(sock, SOL_SOCKET, SO_REUSEPORT, &enable, sizeof(int)) < 0)
    perror("setsockopt(SO_REUSEADDR) failed");

  struct sockaddr_in addr;
  bzero((char *)&addr, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = inet_addr(EXAMPLE_GROUP);
  addr.sin_port = htons(EXAMPLE_PORT);
  addrlen = sizeof(addr);

  struct utsname buf;
  uname(&buf);


  blockDevicesType *bd = blockDevicesInit();

  blockDevicesScan(bd);

  // find unique hw.mac
  interfacesIntType *n = interfacesInit();
  interfacesScan(n);
  char *uniquemac = interfacesOnboardMac(n);
  interfacesFree(n);

  
  while (keepRunning) {
    double now = timeAsDouble();


    keyvalueType *kv = keyvalueInit();
    keyvalueSetString(kv, "action", "hello");
    keyvalueSetString(kv, "node", uniquemac);
    keyvalueSetString(kv, "hostname", buf.nodename);
    keyvalueSetString(kv, "nodename", buf.nodename);
    char *nodeos = OSRelease();
    //    fprintf(stderr,"OS :%s\n", nodeos);
    keyvalueSetString(kv, "nodeOS", nodeos);
    free(nodeos);
    keyvalueSetLong(kv, "time", (long)now);
    keyvalueSetLong(kv, "cluster", cluster->id);
    keyvalueSetLong(kv, "port", 1600);

     size_t hddsize = 0;
     size_t hddnum = blockDevicesCount(bd, "HDD", &hddsize);
     size_t ssdsize = 0;
     size_t ssdnum = blockDevicesCount(bd, "SSD", &ssdsize);

     keyvalueSetLong(kv, "HDDcount", hddnum);
     keyvalueSetLong(kv, "HDDsizeGB", hddsize / 1000.0 / 1000 / 1000);;
     keyvalueSetLong(kv, "SSDcount", ssdnum);
     keyvalueSetLong(kv, "SSDsizeGB", ssdsize / 1000.0 / 1000 / 1000);;

     keyvalueSetLong(kv, "Cores", getNumHardwareThreads());
     keyvalueSetLong(kv, "RAMGB", totalRAM()/1024/1024/1024);

    
    //    keyvalueSetString(kv, "shell", "stush");
    keyvalueSetLong(kv, "started", (long)starttime);
    char *message = keyvalueDumpAsString(kv);
    //    fprintf(stderr,"mc:%s\n", message);
    cnt = sendto(sock, message, strlen(message), 0,
		 (struct sockaddr *) &addr, addrlen);
    if (cnt < 0) {
      perror("sendto");
    }
    free(message);
    keyvalueFree(kv);
    


    count++;
    sleep (cluster->id < 1 ? 1 : cluster->id);
  }
  return NULL;
}
