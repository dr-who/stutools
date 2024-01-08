/* 

multicast.c

The following program sends or receives multicast packets. If invoked
with one argument, it sends a packet containing the current time to an
arbitrarily chosen multicast group and UDP port. If invoked with no
arguments, it receives and prints these packets. Start it as a sender on
just one host and as a receiver on all the other hosts

*/

#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <time.h>
#include <stdio.h>
#include <stdio.h>
#include <strings.h>
#include <unistd.h>
#include <stdlib.h>
#include <assert.h>

#include "respond-mc.h"
#include "multicast.h"


#include "cluster.h"
#include "simpmail.h"


void *respondMC(void *arg) {
  printf("*** RESPOND MC\n");
  if (arg) {}

  clusterType *cluster = clusterInit(1600);

  struct sockaddr_in addr;
   int sock, cnt;
   socklen_t addrlen;
   struct ip_mreq mreq;
   char *message = calloc(100, 1); assert(message);

   /* set up socket */
   sock = socket(AF_INET, SOCK_DGRAM, 0);
   if (sock < 0) {
     perror("socket");
     //     exit(1);
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
     //     exit(1);
   }    
   mreq.imr_multiaddr.s_addr = inet_addr(EXAMPLE_GROUP);         
   mreq.imr_interface.s_addr = htonl(INADDR_ANY);         
   if (setsockopt(sock, IPPROTO_IP, IP_ADD_MEMBERSHIP,
		  &mreq, sizeof(mreq)) < 0) {
     perror("setsockopt mreq");
     //     exit(1);
   }         
   while (1) {
     cnt = recvfrom(sock, message, 100, 0, 
		    (struct sockaddr *) &addr, &addrlen);
     if (cnt < 0) {
       perror("recvfrom");
       //       exit(1);
     } else if (cnt == 0) {
       break;
     }

     printf("**NEW**  should try and connect to '%s' message = \"%s\"\n", inet_ntoa(addr.sin_addr), message);

     double startedtime = 0, expires = 0, senttime = 0;
     char *node = inet_ntoa(addr.sin_addr);
     int nodeid = 0;

     if (sscanf(message, "%*s %lf %lf %lf", &senttime, &startedtime,  &expires) == 3) {
       // got good info
     }
     
     if ((nodeid = clusterFindNode(cluster, node)) < 0) {
       // add and say hi
       nodeid = clusterAddNode(cluster, node, startedtime);
     }

     printf("updating nodeid %d\n", nodeid);
     clusterSetNodeExpires(cluster, nodeid, expires);
     clusterSetNodeIP(cluster, nodeid, node);
       
     
     clusterDumpJSON(stderr, cluster);
     
     int sock = sockconnect(node, 1600, 5);
     if (sock > 0) {
       socksend(sock, "Hello\n", 0, 0);
     }
     close(sock);
   }
 return NULL;
}

