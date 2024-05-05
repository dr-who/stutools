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
  volatile threadMsgType *tc = (threadMsgType *) arg;

    clusterType *cluster = tc->cluster;
  
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
  addr.sin_port = htons(tc->serverport);
  addrlen = sizeof(addr);

  struct utsname buf;
  uname(&buf);



  // find unique hw.mac
  //  interfacesIntType *n = interfacesInit();
  //  interfacesScan(n);
  char *uniquemac = NULL, *adv_ip = NULL;

  unsigned long monotonic = 0;

  double lastfull = 0, lastbeacon = 0;
  
  while (keepRunning) {
    double now = timeAsDouble();
    size_t hddsize = 0, hddnum = 0, ssdsize= 0, ssdnum = 0, volatileramsize = 0, volatileramnum = 0;

    if (now - lastfull > 60) { // run first time
      // every 60s scan
      blockDevicesType *bd = blockDevicesInit();
      blockDevicesScan(bd);
      hddsize = 0;
      hddnum = blockDevicesCount(bd, "HDD", &hddsize);
      ssdsize = 0;
      ssdnum = blockDevicesCount(bd, "SSD", &ssdsize);
      volatileramsize = 0;
      volatileramnum = blockDevicesCount(bd, "Volatile-RAM", &volatileramsize);

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
    keyvalueSetLong(kv, "mono", monotonic);
    keyvalueSetString(kv, "hostname", buf.nodename);
    keyvalueSetString(kv, "nodename", buf.nodename);
    char *hwtype = hwMachine();
    keyvalueSetString(kv, "nodeHW", hwtype); // wil ldup
    
    char *nodeos = OSRelease();
    //    fprintf(stderr,"OS :%s\n", nodeos);
    keyvalueSetString(kv, "nodeOS", nodeos); // will dup

    char str[1000];
    sprintf(str, "/sys/devices/virtual/dmi/id/board_name");
    char *boardvn = getStringFromFile(str, 1);
    keyvalueSetString(kv, "boardname", boardvn ? boardvn : "");
    free(boardvn);

    sprintf(str, "/sys/devices/virtual/dmi/id/bios_date");
    char *biosdate = getStringFromFile(str, 1);

    keyvalueSetString(kv, "biosdate", biosdate ? biosdate : "");
    free(biosdate);



    
    keyvalueSetLong(kv, "time", (long)now);
    keyvalueSetLong(kv, "cluster", cluster->id);
    keyvalueSetLong(kv, "port", tc->serverport);

     keyvalueSetLong(kv, "HDDcount", hddnum);
     keyvalueSetLong(kv, "HDDsizeGB", ceil(hddsize / 1000.0 / 1000 / 1000));
     keyvalueSetLong(kv, "SSDcount", ssdnum);
     keyvalueSetLong(kv, "SSDsizeGB", ceil(ssdsize / 1000.0 / 1000 / 1000));
     keyvalueSetLong(kv, "VolatileRAMcount", volatileramnum);
     keyvalueSetLong(kv, "VolatileRAMsizeGB", ceil(volatileramsize / 1000.0 / 1000 / 1000));

     keyvalueSetLong(kv, "Cores", getNumHardwareThreads());
     keyvalueSetLong(kv, "RAMGB", ceil(totalRAM()/1024.0/1024/1024));

     keyvalueSetString(kv, "ip", adv_ip);

     //     fprintf(stderr,"%s\n", keyvalueDumpAsString(kv));
     
    
    //    keyvalueSetString(kv, "shell", "stush");
    keyvalueSetLong(kv, "started", (long)starttime);
    char *message = keyvalueDumpAsString(kv);


    if (now - lastfull >15) { 
      lastfull = now;
      fprintf(stderr,"[full] %s\n", message);
      cnt = sendto(sock, message, strlen(message), 0, (struct sockaddr *) &addr, addrlen);
      if (cnt < 0) {
	perror("sendto");
      }
      monotonic++;
    } else if (now - lastbeacon > 5) {
      sprintf(message,"node:%s nodename:bob time;%ld mono;%ld", buf.nodename, (long)now, keyvalueGetLong(kv, "mono"));
      lastbeacon = now;
      fprintf(stderr,"[beacon] %s\n", message);
      cnt = sendto(sock, message, strlen(message), 0, (struct sockaddr *) &addr, addrlen);
      if (cnt < 0) {
	perror("sendto");
      }
      monotonic++;

    }


    free(message);
    keyvalueFree(kv);
    
    double taken = timeAsDouble() - now;
    if (taken > 5) taken = 5;

    fprintf(stderr,"sleeping for %.2lf\n", 5-taken);
    sleep(5 - taken);
  }
  return NULL;
}
