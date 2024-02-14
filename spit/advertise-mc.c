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

  // find unique hw.mac
  //  interfacesIntType *n = interfacesInit();
  //  interfacesScan(n);
  char *uniquemac, *adv_ip;

  double last = 0; 
  while (keepRunning) {
    double now = timeAsDouble();

    if (now - last > 60) { // run first time
      // every 60s scan
      blockDevicesScan(bd);
      interfacesIntType *n = interfacesInit();
      interfacesScan(n);
      free(adv_ip);
      uniquemac = interfacesOnboardHW(1);
      adv_ip = interfaceIPNonWifi(n); // pickup a changing main IP
      interfacesFree(&n); assert(n==NULL);
      
    }


    keyvalueType *kv = keyvalueInit();
    keyvalueSetString(kv, "action", "hello");
    keyvalueSetString(kv, "node", uniquemac);
    keyvalueSetString(kv, "hostname", buf.nodename);
    keyvalueSetString(kv, "nodename", buf.nodename);
    char *nodeos = OSRelease();
    //    fprintf(stderr,"OS :%s\n", nodeos);
    keyvalueSetString(kv, "nodeOS", strdup(nodeos));
    free(nodeos);

    char str[1000];
    sprintf(str, "/sys/devices/virtual/dmi/id/board_name");
    char *boardvn = getStringFromFile(str, 1);
    keyvalueSetString(kv, "boardname", boardvn);
    free(boardvn);

    sprintf(str, "/sys/devices/virtual/dmi/id/bios_date");
    char *biosdate = getStringFromFile(str, 1);

    keyvalueSetString(kv, "biosdate", biosdate);
    free(biosdate);



    
    keyvalueSetLong(kv, "time", (long)now);
    keyvalueSetLong(kv, "cluster", cluster->id);
    keyvalueSetLong(kv, "port", 1600);

     size_t hddsize = 0;
     size_t hddnum = blockDevicesCount(bd, "HDD", &hddsize);
     size_t ssdsize = 0;
     size_t ssdnum = blockDevicesCount(bd, NULL, &ssdsize);

     keyvalueSetLong(kv, "HDDcount", hddnum);
     keyvalueSetLong(kv, "HDDsizeGB", hddsize / 1000.0 / 1000 / 1000);;
     keyvalueSetLong(kv, "SSDcount", ssdnum);
     keyvalueSetLong(kv, "SSDsizeGB", ssdsize / 1000.0 / 1000 / 1000);;

     keyvalueSetLong(kv, "Cores", getNumHardwareThreads());
     keyvalueSetLong(kv, "RAMGB", totalRAM()/1024/1024/1024);

     keyvalueSetString(kv, "ip", adv_ip);

     //     fprintf(stderr,"%s\n", keyvalueDumpAsString(kv));
     
    
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
    sleep (cluster->id < 1 ? 1 : (cluster->id)); // 2 * nyquist
  }
  return NULL;
}
