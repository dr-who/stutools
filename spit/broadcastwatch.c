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
#include <stdlib.h>
#include <string.h>
#include <unistd.h>


#include "multicast.h"

#define MESSAGELEN 1024

int main(int argc, char *argv[]) {
   struct sockaddr_in addr;
   socklen_t addrlen;
   int sock, cnt;
   struct ip_mreq mreq;
   char message[MESSAGELEN+1]; // add one for \0

   if (argv) {}

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


   bzero((char *)&addr, sizeof(addr));
   addr.sin_family = AF_INET;
   addr.sin_addr.s_addr = htonl(INADDR_ANY);
   addr.sin_port = htons(EXAMPLE_PORT);
   addrlen = sizeof(addr);

   if (argc > 1) {
      /* send */
      addr.sin_addr.s_addr = inet_addr(EXAMPLE_GROUP);
      while (1) {
	 time_t t = time(0);
	 sprintf(message, "time is %-24.24s", ctime(&t));
	 printf("sending: %s\n", message);
	 cnt = sendto(sock, message, strlen(message), 0,
		      (struct sockaddr *) &addr, addrlen);
	 if (cnt < 0) {
 	    perror("sendto");
	    exit(1);
	 }
	 sleep(5);
      }
   } else {

      /* receive */
      if (bind(sock, (struct sockaddr *) &addr, sizeof(addr)) < 0) {        
         perror("bind");
	 exit(1);
      }    
      mreq.imr_multiaddr.s_addr = inet_addr(EXAMPLE_GROUP);         
      mreq.imr_interface.s_addr = inet_addr("192.168.5.100");         
      if (setsockopt(sock, IPPROTO_IP, IP_ADD_MEMBERSHIP,
		     &mreq, sizeof(mreq)) < 0) {
	 perror("setsockopt mreq");
	 exit(1);
      }         
      while (1) {
	cnt = recvfrom(sock, message, MESSAGELEN, 0, 
			(struct sockaddr *) &addr, &addrlen);
	 if (cnt < 0) {
	    perror("recvfrom");
	    exit(1);
	 } else if (cnt == 0) {
 	    break;
	 } else {
	   message[cnt] = '\0';
	 }

	 printf("%s: '%s' %d\n", inet_ntoa(addr.sin_addr), message, cnt);
        }
    }

   return 0;
}

