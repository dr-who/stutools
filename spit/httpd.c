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
#include <netdb.h>
#include <ifaddrs.h>
#include <signal.h>
#include <sys/utsname.h>

#include "transfer.h"

#include "utils.h"

#include <glob.h>

#include "advertise-mc.h"
#include "respond-mc.h"
#include "blockdevices.h"

int keepRunning = 1;
int tty = 0;

void intHandler(int d) {
    if (d) {}
    fprintf(stderr, "got signal\n");
    keepRunning = 0;
    exit(-1);
}


#include "sat.h"
#include "snack.h"
#include "interfaces.h"
#include "cluster.h"
#include "iprange.h"
#include "ipcheck.h"
#include "simpsock.h"
#include "keyvalue.h"

 
void *receiver(void *arg) {
  threadMsgType *tc = (threadMsgType *) arg;


  int serverport = tc->serverport;

  int sockfd = socket(AF_INET, SOCK_STREAM, 0);
  if (sockfd == -1) {
    perror("Can't allocate sockfd");
    //	  continue;
  }

  //	socksetup(sockfd, 10);
  int true = 1;
  if (setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &true, sizeof(int)) == -1) {
    perror("so_reuseaddr");
    close(sockfd);
    exit(1);
  }
  if (setsockopt(sockfd, SOL_SOCKET, SO_REUSEPORT, &true, sizeof(int)) == -1) {
    perror("so_reuseport");
    close(sockfd);
    exit(1);
  }

  struct sockaddr_in clientaddr, serveraddr;
  memset(&serveraddr, 0, sizeof(serveraddr));
  serveraddr.sin_family = AF_INET;
  serveraddr.sin_addr.s_addr = htonl(INADDR_ANY);
  serveraddr.sin_port = htons(serverport);

  if (bind(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) == -1) {
    perror("Bind Error");
    close(sockfd);
    exit(1);
    //	  continue;
  }

  if (listen(sockfd, 200) == -1) {
    perror("Listen Error");
    close(sockfd);
    exit(1);
    //	  continue;
  }

  srand(time(NULL));

  int connections = 0;
  size_t bytes = 0;
  double start =timeAsDouble();

#define THESZ (1024*1024)
      
  char *databuf = calloc(THESZ,1);
  char ch = 'A';
  
  for (size_t ii = 0; ii < THESZ; ii++) {
    databuf[ii] = ch++;
  }
  
  while (keepRunning) {
    socklen_t addrlen = sizeof(clientaddr);
    int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
    if (connfd == -1) {
      perror("Connect Error");
      exit(1);
    }

    //    char addr[INET_ADDRSTRLEN];
    //    inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN);

      char buffer[1024];
      if (sockrec(connfd, buffer, 1024, 0, 1) < 0) {
	perror("sockrec connfd");
	break;
	//	continue;
	//	  fprintf(stderr, "din'yt get a string\n");
      }

      /*      char print[100];
      memset(print, 0, 100);
      strncpy(print, buffer, 99);
      fprintf(stderr,"-->%s\n", print);
      */

      char *sendbuffer = calloc(THESZ+200,1);
      
      sprintf(sendbuffer, "HTTP/1.1 200 OK\nDate: Sun, 28 Jan 2024 04:57:01 GMT\nConnection: close\nCache-Control: no-store, no-cache, must-revalidate\nContent-Length: %d\n", THESZ);
      char s[100];
      sprintf(s, "X-Position: %zd\n", rand());
      strcat(sendbuffer, s);
      strcat(sendbuffer, "\n");
      const size_t pos = strlen(sendbuffer);      
      memcpy(sendbuffer+pos, databuf, THESZ);
      
      const int contentLen = pos + THESZ;

      connections++;
      bytes += contentLen;
      fprintf(stderr,"%d  %zd  %.1lf/s\n", connections, bytes/1024/1024, bytes*1.0/1024/1024/(timeAsDouble()-start));
      
      if (fcntl(connfd, F_GETFD) == -1) break;
      if (socksendWithLen(connfd, sendbuffer, contentLen, 0, 1) < 0) {
	perror("sock send\n");
	break;
      }
      free(sendbuffer);
    
    close(connfd);
  }
  close(sockfd);
  return NULL;
}


void startThreads(interfacesIntType *n, const int serverport) {
  fprintf(stderr,"**start** httpd on  ->  port %d\n", serverport);

    pthread_t *pt;
    threadMsgType *tc;
    size_t num = 1; // threads

    CALLOC(pt, num, sizeof(pthread_t));
    CALLOC(tc, num, sizeof(threadMsgType));

    clusterType *cluster = clusterInit(serverport);

    for (size_t i = 0; i < num; i++) {
        tc[i].id = i;
        tc[i].num = num;
        tc[i].serverport = serverport;
	tc[i].n = n;
        tc[i].starttime = timeAsDouble();
	tc[i].cluster = cluster;
	tc[i].eth = NULL; 
	pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
    }

    for (size_t i = 0; i < num; i++) {
      pthread_join(pt[i], NULL);
      printf("thread %zd finished %d\n", i, keepRunning);
    }
    free(tc);
    free(pt);
}






int main() {
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  interfacesIntType *n = interfacesInit();
  interfacesScan(n);

  int port = 8080;
  if (getenv("SAT_PORT")) {
    port = atoi(getenv("SAT_PORT"));
    fprintf(stderr,"systemd environment SAT_PORT is %d\n", port);
    if (port < 1) port = 8080;
  }

  startThreads(n, port);

  return 0;
}
