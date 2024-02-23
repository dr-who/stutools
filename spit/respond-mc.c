/* 

multicast.c


*/

#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <time.h>
#include <stdio.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <stdlib.h>
#include <assert.h>

#include "respond-mc.h"
#include "multicast.h"
#include "utilstime.h"

#include "cluster.h"
#include "simpsock.h"
#include "keyvalue.h"
#include "sat.h"


void *respondMC(void *arg) {
  volatile threadMsgType *tc = (threadMsgType *) arg;

  clusterType *cluster = tc->cluster;

  struct sockaddr_in addr;
   int sock, cnt;
   socklen_t addrlen;
   struct ip_mreq mreq;
   char *message = calloc(300, 1); assert(message);

   /* set up socket */
   sock = socket(AF_INET, SOCK_DGRAM, 0);
   fprintf(stderr,"opening socket %d\n", sock);
   if (sock < 0) {
     perror("socket");
     exit(1);
   }
   
   const int enable = 1;
   if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &enable, sizeof(int)) < 0)
     perror("setsockopt(SO_REUSEADDR) failed");
   if (setsockopt(sock, SOL_SOCKET, SO_REUSEPORT, &enable, sizeof(int)) < 0)
     perror("setsockopt(SO_REUSEADDR) failed");
   
  bzero((char *)&addr, sizeof(addr));
   addr.sin_family = AF_INET;
   addr.sin_addr.s_addr = htonl(INADDR_ANY);
   addr.sin_port = htons(EXAMPLE_PORT);
   addrlen = sizeof(addr);

   
   /* receive */
   if (bind(sock, (struct sockaddr *) &addr, sizeof(addr)) < 0) {        
     perror("bind");
          exit(1);
   }
   printf("binding\n");
   mreq.imr_multiaddr.s_addr = inet_addr(EXAMPLE_GROUP);         
   mreq.imr_interface.s_addr = htonl(INADDR_ANY);         
   if (setsockopt(sock, IPPROTO_IP, IP_ADD_MEMBERSHIP,
		  &mreq, sizeof(mreq)) < 0) {
     perror("setsockopt mreq");
          exit(1);
   }
   //   double last = timeAsDouble();

   // initial block device scan


   size_t nodesGood = 0, nodesBad = 0, nodesLastNum = 0;
   double nodeBadLastCheck = 0, alertPingTime = timeAsDouble(), thistime = 0;
   

   assert(cluster->localsmtp);
   assert(cluster->alertToEmail);
   socksetup(sock, 10);
   while (1) {
     // once every 2 mins or 10 seconds if one bad
     double checktime = 120;
     if (nodesBad) checktime = 10;

     size_t shouldPing = 0;
     thistime = timeAsDouble();
     if (thistime - alertPingTime >= 3600*24) { // once a day send an update
       shouldPing = 1;
       alertPingTime += 3600*24; // increase it by a day

     }
       
     
     if ((thistime - nodeBadLastCheck > checktime) || shouldPing) {
       nodeBadLastCheck = thistime;

       // calc good and bad
       char *bad = clusterGoodBad(cluster, &nodesGood, &nodesBad);
       free(bad);

       // if change in bad ALERT, any change or it's been a while
       if (shouldPing | (nodesGood - nodesBad != nodesLastNum)) {
	 if (shouldPing) {
	   fprintf(stderr,"*info* regular PING alerts\n");
	 } else {
	   fprintf(stderr,"*info* change in good/bad alert\n");
	   nodesLastNum = nodesGood - nodesBad;
	 }
	 clusterSendAlertEmail(cluster);
       }
       // log once a min
       fprintf(stderr,"node status: good: %zd, bad: %zd\n", nodesGood, nodesBad);
     }
     
     cnt = recvfrom(sock, message, 300, 0, 
		    (struct sockaddr *) &addr, &addrlen);
     if (cnt < 0) {
       fprintf(stderr, "no broadcast is observed, even though this service is running. open port %d/UDP\n", EXAMPLE_PORT);
       continue;
       //       exit(1);
     } else if (cnt == 0) {
       break;
     }
     if (cnt < 300) message[cnt] = 0; // make it terminated nicely

     //          fprintf(stderr, "**NEW**  should try and connect to '%s' message = \"%s\"\n", inet_ntoa(addr.sin_addr), message);

     long startedtime = 0;//
     char *ipaddr = inet_ntoa(addr.sin_addr);

     //fprintf(stderr,"rec:%s\n", message);
     
     keyvalueType *kv = keyvalueInitFromString(message);
     
     if (kv->num >= 2) {
       //       char *ss = keyvalueDumpAsJSON(kv);
       //              fprintf(stderr,"%s\n", ss);
       //	             free(ss);
       
       int nodeid = 0; 

       //       int port = keyvalueGetLong(kv, "port");
       char *nodename = keyvalueGetString(kv, "node");

       if (nodename) {
	 //     senttime = keyvalueGetLong(kv, "time");
	 startedtime = keyvalueGetLong(kv, "started");
	 char *hostname = keyvalueGetString(kv, "nodename");
	 if (hostname == NULL) {
	   hostname = keyvalueGetString(kv, "hostname");
	 }
	 assert(hostname);

	 
	 
	 if ((nodeid = clusterFindNode(cluster, nodename)) < 0) {
	   // add and say hi
	   fprintf(stderr, "adding node %s (%s) (%s)\n", nodename, hostname, ipaddr);
	   nodeid = clusterAddNode(cluster, nodename, startedtime);
	 }
	 free(cluster->node[nodeid]->nodename);
	 cluster->node[nodeid]->nodename = strdup(hostname); // manually added so strdup
	 
	 cluster->node[nodeid]->nodeOS = keyvalueGetString(kv, "nodeOS");
	 cluster->node[nodeid]->boardname = keyvalueGetString(kv, "boardname");
	 cluster->node[nodeid]->biosdate = keyvalueGetString(kv, "biosdate");
	 cluster->node[nodeid]->HDDcount = keyvalueGetLong(kv, "HDDcount");
	 cluster->node[nodeid]->HDDsizeGB= keyvalueGetLong(kv, "HDDsizeGB");
	 cluster->node[nodeid]->SSDcount = keyvalueGetLong(kv, "SSDcount");
	 cluster->node[nodeid]->SSDsizeGB = keyvalueGetLong(kv, "SSDsizeGB");
	 cluster->node[nodeid]->RAMGB = keyvalueGetLong(kv, "RAMGB");
	 cluster->node[nodeid]->Cores = keyvalueGetLong(kv, "Cores");
	 
	 if (keyvalueSetLong(tc->cluster->node[nodeid]->info, "cluster", keyvalueGetLong(kv, "cluster"))) {
	   clusterChanged(cluster, nodeid);
	 }
	 if (keyvalueSetString(tc->cluster->node[nodeid]->info, "ip", keyvalueGetString(kv, "ip"))) {
	   clusterChanged(cluster, nodeid);
	 }
	 if (keyvalueSetString(tc->cluster->node[nodeid]->info, "nodename", keyvalueGetString(kv, "nodename"))) {
	   clusterChanged(cluster, nodeid);
	 }
	 if ( keyvalueSetString(tc->cluster->node[nodeid]->info, "nodeOS", keyvalueGetString(kv, "nodeOS"))) {
	   clusterChanged(cluster, nodeid);
	 }
	 
	 keyvalueFree(kv);
	 
	 clusterUpdateSeen(cluster, nodeid);
       }
     } else {
       fprintf(stderr,"problem parsing: %s\n", message);
     }
   } // while 1
 return NULL;
}

