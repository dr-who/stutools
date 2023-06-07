#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <arpa/inet.h>
#include <time.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <assert.h>

#include "transfer.h"

#include "utils.h"
#include "numList.h"

#include <glob.h>

#include "dns.h"

void dnsConnect(char *ipaddress) {
  assert(ipaddress);

  int sockfd = socket(AF_INET, SOCK_DGRAM, 0);
  if (sockfd < 0) {
    perror("Can't allocate sockfd");
    return;
  }
  
  struct sockaddr_in serveraddr;
  memset(&serveraddr, 0, sizeof(serveraddr));
  long c = getDevRandomLong();
  srand(c);
  
  int port = 53;
  serveraddr.sin_family = AF_INET;
  serveraddr.sin_port = htons(port);

  if (inet_aton(ipaddress, &serveraddr.sin_addr) < 0) {
    perror("IPaddress Convert Error");
    return;
  }
  
  if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
    perror("Listening port not available");
  } else {
    printf("connected\n");
  }


  unsigned char buf[512];
  
  int n = send(sockfd, buf, 512, 0);
  printf("sent: %d\n", n);
  
  fprintf(stderr,"*info* connected to %s on port %d\n", ipaddress, port);
  
  close(sockfd);
  return;
}

