/* 

// base from online

multicast.c

The following program sends or receives multicast packets. If invoked
with one argument, it sends a packet containing the current time to an
arbitrarily chosen multicast group and UDP port. If invoked with no
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
#include <strings.h>
#include <unistd.h>
#include <stdlib.h>
#include "utilstime.h"


#include "advertise-mc.h"
#include "multicast.h"

void *advertiseMC(void *arg) {
  if (arg) {}
  
  struct sockaddr_in addr;
  int count = 0;
  int addrlen, sock, cnt;
  char message[100];
  double startTime = timeAsDouble();

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

  /* send */
  addr.sin_addr.s_addr = inet_addr(EXAMPLE_GROUP);
  while (keepRunning) {
    const double now = timeAsDouble();
    
    sprintf(message, "time:%.0lf uptime:%.0lf port:1600 stush", now, now - startTime);
    cnt = sendto(sock, message, sizeof(message), 0,
		 (struct sockaddr *) &addr, addrlen);
    if (cnt < 0) {
      perror("sendto");
      //      exit(1);
    }

    count++;
    sleep(count > 10?10:count);
  }
  return NULL;
}
