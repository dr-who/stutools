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
    threadMsgType *tc = (threadMsgType *) arg;

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

   
   while (1) {
     cnt = recvfrom(sock, message, 300, 0, 
		    (struct sockaddr *) &addr, &addrlen);
     if (cnt < 0) {
       perror("recvfrom");
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
	   cluster->node[nodeid]->nodename= strdup(hostname); // manually added so strdup
	   cluster->node[nodeid]->nodeOS = keyvalueGetString(kv, "nodeOS");
	   cluster->node[nodeid]->boardname = keyvalueGetString(kv, "boardname");
	   cluster->node[nodeid]->biosdate = keyvalueGetString(kv, "biosdate");
	   cluster->node[nodeid]->HDDcount = keyvalueGetLong(kv, "HDDcount");
	   cluster->node[nodeid]->HDDsizeGB= keyvalueGetLong(kv, "HDDsizeGB");
	   cluster->node[nodeid]->SSDcount = keyvalueGetLong(kv, "SSDcount");
	   cluster->node[nodeid]->SSDsizeGB = keyvalueGetLong(kv, "SSDsizeGB");
	   
	   cluster->node[nodeid]->RAMGB = keyvalueGetLong(kv, "RAMGB");
	   cluster->node[nodeid]->Cores = keyvalueGetLong(kv, "Cores");
	 }


	 char *keyip = keyvalueGetString(kv, "ip");
	 if (keyip && (strstr(keyip, ipaddr) == NULL)) {
	   fprintf(stderr,"warning; %s connecting IP (%s) different from multicast IP (%s). Using multicast IP\n", hostname, ipaddr, keyip);
	   ipaddr = keyip;
	 } else {
	   free(keyip);
	 }
	 
	 
	 if (strcmp(clusterGetNodeIP(cluster, nodeid), ipaddr) != 0) {
	   //	   fprintf(stderr, "updating nodeid %d, %s IP %s: '^%s'\n", nodeid, cluster->node[nodeid]->name, node, message);
	   clusterSetNodeIP(cluster, nodeid, ipaddr);
	   if (cluster->node[nodeid]->nodename) {
	     free(cluster->node[nodeid]->nodename);
	   }
	   cluster->node[nodeid]->nodename= strdup(hostname); // manually added so strdup

	   // check OS upgrade
	   char *broad_os = keyvalueGetString(kv, "nodeOS"); // get creates
	   int doupdate = 0;
	   if (broad_os)  { // if we have a broadcast value
	     doupdate = 1;
	     if (cluster->node[nodeid]->nodeOS) {
	       if (strcmp(broad_os, cluster->node[nodeid]->nodeOS) == 0) { // if same don't update
		 doupdate = 0;
	       }
	     }
	     if (doupdate) {
	       free(cluster->node[nodeid]->nodeOS);
	       cluster->node[nodeid]->nodeOS = broad_os;
	     }
	   } // got a broadcast
		      
	   
	 }

	 keyvalueFree(kv);

	 
	 clusterUpdateSeen(cluster, nodeid);

	 //	 clusterDumpJSON(stdout, cluster);

	   // hello every 10 seconds
	 //	   last = timeAsDouble();
	 /*	   int nsock = sockconnect(node, port, 30);
	   if (nsock > 0) {
	     socksetup(nsock, 10);
	     char buff[100000];
	     socksend(nsock, "Hello\n", 0, 1);
	     sockrec(nsock, buff, 100000, 0, 1);
	     //	     printf("res:%s\n", buff);
	     close(nsock);
	     }*/
       }
     } else {
       fprintf(stderr,"problem parsing: %s\n", message);
     }
   } // while 1
 return NULL;
}

