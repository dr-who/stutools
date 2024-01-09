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

 
void *receiver(void *arg) {
    threadMsgType *tc = (threadMsgType *) arg;

    while (keepRunning) {
        int serverport = tc->serverport;

        int sockfd = socket(AF_INET, SOCK_STREAM, 0);
        if (sockfd == -1) {
	  //            perror("Can't allocate sockfd");
	    continue;
        }

        int true = 1;
        if (setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &true, sizeof(int)) == -1) {
	  //            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
	//	if (socksetup(sockfd, 30) < 0) {
	  //
	//	  close(sockfd);
	//	  continue;
	//	}

        struct sockaddr_in clientaddr, serveraddr;
        memset(&serveraddr, 0, sizeof(serveraddr));
        serveraddr.sin_family = AF_INET;
        serveraddr.sin_addr.s_addr = htonl(INADDR_ANY);
        serveraddr.sin_port = htons(serverport);

        if (bind(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) == -1) {
	  //            perror("Bind Error");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }

        if (listen(sockfd, serverport) == -1) {
	  //            perror("Listen Error");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }

        socklen_t addrlen = sizeof(clientaddr);
        int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
	if (connfd == -1) {
	  //	  perror("Connect Error");
	  close(sockfd);
	  continue;
	}
	    //            exit(1);
	    //        }
        char addr[INET_ADDRSTRLEN];
        inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN);

	char buffer[1024];
	if (sockrec(connfd, buffer, 1024, 0, 1) < 0) {
	  fprintf(stderr, "din'yt get a string\n");
	  goto end;
	}

	//	fprintf(stderr, "received a string: %s\n", buffer);
	
	if (strncmp(buffer,"Hello",5)==0) {
	  char s1[100],ipa[100];
	  //	  printf("buffer: %s\n", buffer);
	  sscanf(buffer,"%*s %s %s", s1,ipa);
	  //	  fprintf(stderr,"*server says it's a welcome/valid client ... %s %s\n", s1, ipa);

	  struct utsname buf;
	  uname(&buf);
	  sprintf(buffer,"Hello %s %s\n", buf.nodename, ipa);
	  if (socksend(connfd, buffer, 0, 1) < 0)
	    goto end;
	} else if (strncmp(buffer,"interfaces",10)==0) {
	  char *json = interfacesDumpJSONString(tc->n);
	  int err = socksend(connfd, json, 0, 1);
	  free(json);
	  if (err < 0) goto end;
	} else if (strncmp(buffer,"cluster",7)==0) {
	  fprintf(stderr, "sending cluster info back: %zd nodes\n", tc->cluster->id);
	  char *json = clusterDumpJSONString(tc->cluster);
	  socksend(connfd, json, 0, 1);
	  socksend(connfd, "\n", 0, 1);
	  free(json);
	} else {
	  //	    fprintf(stderr,"*unknown handshake, invalid client\n");
	  sleep(1);
	}
	
	//	fprintf(stderr,"end of server loop\n");
	

    end:
	  
	//	shutdown(sockfd, SHUT_RDWR);
	close(sockfd);
	
	close(connfd);
    }
    return NULL;
}


void startThreads(interfacesIntType *n, const int serverport) {
  fprintf(stderr,"**start** Stu's Autodiscover Tool (sat)  ->  port %d\n", serverport);

    pthread_t *pt;
    threadMsgType *tc;
    size_t num = 3; // threads

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
	if (i == 0) {
	  pthread_create(&(pt[i]), NULL, respondMC, &(tc[i]));
	} else if (i == 1) {
	  pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
	} else if (i == 2) {
	  pthread_create(&(pt[i]), NULL, advertiseMC, &(tc[i]));
	}
    }

    for (size_t i = 0; i < num; i++) {
      pthread_join(pt[i], NULL);
      //	printf("thread %zd finished %d\n", i, keepRunning);
    }
    free(tc);
    free(pt);
}






int main() {
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  interfacesIntType *n = interfacesInit();
  interfacesScan(n);

  int port = 1600;
  if (getenv("SAT_PORT")) {
    port = atoi(getenv("SAT_PORT"));
    fprintf(stderr,"systemd environment SAT_PORT is %d\n", port);
    if (port < 1) port = 1600;
  }

  startThreads(n, port);

  return 0;
}
