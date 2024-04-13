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
   addr.sin_port = htons(tc->serverport);
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
       fprintf(stderr, "no broadcast is observed, even though this service is running. open port %d/UDP\n", tc->serverport);
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
       char *node = keyvalueGetString(kv, "node"); // UUNIX

       if (node) {
	 //     senttime = keyvalueGetLong(kv, "time");
	 startedtime = keyvalueGetLong(kv, "started");

	 if ((nodeid = clusterFindNode(cluster, node)) < 0) {
	   // add and say hi
	   fprintf(stderr, "adding node %s (%s)\n", node, ipaddr);
	   nodeid = clusterAddNode(cluster, node, startedtime);
	 }

	 char *tmp = keyvalueGetString(kv, "nodename"); // human readable
	 keyvalueSetString(tc->cluster->node[nodeid]->info, "nodename", tmp);
	 free(tmp);

	 tmp = keyvalueGetString(kv, "nodeHW"); // human readable
	 keyvalueSetString(tc->cluster->node[nodeid]->info, "nodehw", tmp ? tmp : "");
	 free(tmp);

	 keyvalueSetString(tc->cluster->node[nodeid]->info, "ip", ipaddr);
	 
	 tmp=keyvalueGetString(kv, "nodeOS");
	 keyvalueSetString(tc->cluster->node[nodeid]->info, "nodeOS", tmp);
	 free(tmp);

	 tmp=keyvalueGetString(kv, "boardname");
	 keyvalueSetString(tc->cluster->node[nodeid]->info, "boardname", tmp);
	 free(tmp);

	 tmp=keyvalueGetString(kv, "biosdate");
	 keyvalueSetString(tc->cluster->node[nodeid]->info, "biosdate", tmp);
	 free(tmp);

	 keyvalueSetLong(tc->cluster->node[nodeid]->info, "HDDcount", keyvalueGetLong(kv, "HDDcount"));
	 keyvalueSetLong(tc->cluster->node[nodeid]->info, "HDDsizeGB", keyvalueGetLong(kv, "HDDsizeGB"));
	 keyvalueSetLong(tc->cluster->node[nodeid]->info, "SSDcount", keyvalueGetLong(kv, "SSDcount"));
	 keyvalueSetLong(tc->cluster->node[nodeid]->info, "SDDsizeGB", keyvalueGetLong(kv, "SSDsizeGB"));

	 keyvalueSetLong(tc->cluster->node[nodeid]->info, "RAMGB", keyvalueGetLong(kv, "RAMGB"));
	 keyvalueSetLong(tc->cluster->node[nodeid]->info, "Cores", keyvalueGetLong(kv, "Cores"));
	 
	 if (keyvalueSetLong(tc->cluster->node[nodeid]->info, "cluster", keyvalueGetLong(kv, "cluster"))) {
	   clusterChanged(cluster, nodeid);
	 }
	 // insert up to the /
	 char *br_ip = keyvalueGetString(kv, "ip");
	 if (br_ip) {
	   char *slash = strchr(br_ip, '/');
	   if (slash) {
	     *slash = 0; // \0
	   }
	   if (keyvalueSetString(tc->cluster->node[nodeid]->info, "ip", br_ip)) {
	     clusterChanged(cluster, nodeid);
	   }
	   free(br_ip);
	 } // end ip
	 
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

