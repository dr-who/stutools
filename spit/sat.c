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
#include "transfer.h"

#include "utils.h"
#include "numList.h"

#include <glob.h>

int keepRunning = 1;
int tty = 0;

void intHandler(int d) {
    if (d) {}
    fprintf(stderr, "got signal\n");
    keepRunning = 0;
    exit(-1);
}

char *clusterIPs;

#include "sat.h"
#include "snack.h"
#include "simpmail.h"
#include "interfaces.h"


char * stringListToJSON(char *inc) {
  if (inc == NULL) return NULL;
  
  char *buf = calloc(1,1000000); assert(buf);
  char *ret = buf;

  buf += sprintf(buf, "{\n\t\"servers\": [");
  
  char *s = strdup(inc);
  char *first = NULL, *second = NULL;
  first = strtok(s, " ");

  buf += sprintf(buf, "\"%s\"", first);

  while ((second = strtok(NULL, " "))) {
    buf += sprintf(buf, ", \"%s\"", second);
  }

  buf += sprintf(buf, "] }\n");
  char *ret2 = strdup(ret);
  free(ret);
  return ret2;
}

  
    
  

static void *display(void *arg) {
  //  threadMsgType *d = (threadMsgType*)arg;
  while (keepRunning) {
    if (arg) {
      char s[PATH_MAX];
      sprintf(s, "/proc/%d/fd/", getpid());
      const size_t np = numberOfDirectories(s);
      fprintf(stderr,"*info* openfiles=%zd, cluster: %s\n", np, clusterIPs);
      //      fprintf(stderr,"*display %d*\n", d->id);
      sleep(5);
    }
  }
  return NULL;
}


static void *tryConnect(void *arg) {
  threadMsgType *d = (threadMsgType*)arg;
  while (keepRunning) {

    char *ipaddress = d->tryhost;
    if (strstr(clusterIPs, ipaddress) == 0) {
      sleep(2);
      //      fprintf(stderr,"*info* can't find it, going for it...\n");
    } else {
      fprintf(stderr,"*info* already found %s, sleeping for 30\n", ipaddress);
      sleep(30);
    }

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        perror("Can't allocate sockfd");
	continue;
    }

    struct sockaddr_in serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));

    int port = d->serverport;
    serveraddr.sin_family = AF_INET;
    serveraddr.sin_port = htons(port);
    
    if (inet_aton(ipaddress, &serveraddr.sin_addr) < 0) {
      perror("IPaddress Convert Error");
      close(sockfd);
      continue;
    }

    if (socksetup(sockfd, 20) < 0) {
      perror("sock setup");
      close(sockfd);
      continue;
    }

    
    if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
      //      perror("Listening port not available");
      close(sockfd);
      continue;
    }

    char buff[1024];
    sprintf(buff, "%s", "Hello\n");

    socksend(sockfd, buff, 0, 0);

    sockrec(sockfd, buff, 1024, 0, 0);
    if (strcmp(buff, "World!\n")==0) {
      fprintf(stderr,"*info* client says it's a valid server = %s\n", ipaddress);
      if (strstr(clusterIPs, ipaddress) == NULL) {
	if (clusterIPs[0] != 0) {
	  strcat(clusterIPs, ", ");
	}
	strcat(clusterIPs, ipaddress);
      }
    }

    //    fprintf(stderr,"*info* close and loop\n");
    close(sockfd);
    // all is good, we've connected to something
  }
  return NULL;
}



static void *receiver(void *arg) {
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
	if (socksetup(sockfd, 30) < 0) {
	  //
	  close(sockfd);
	  continue;
	}

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
	if (sockrec(connfd, buffer, 1024, 0, 1) < 0)
	  break;
	
	if (strcmp(buffer,"Hello\n")==0) {
	  fprintf(stderr,"*server says it's a welcome/valid client = %s\n", addr);
	  sprintf(buffer,"World!\n");
	  if (socksend(connfd, buffer, 0, 0) < 0)
	    break;
	} else if (strncmp(buffer,"interfaces",10)==0) {
	  char *json = interfacesDumpJSONString(tc->n);
	  if (socksend(connfd, json, 0, 1) < 0)
	    break;
	  free(json);
	} else if (strncmp(buffer,"cluster",7)==0) {
	  char *json = stringListToJSON(clusterIPs);
	  socksend(connfd, json, 0, 1);
	  socksend(connfd, "\n", 0, 1);
	  free(json);
	} else {
	  sleep(1);
	  //	    fprintf(stderr,"*unknown handshake, invalid client\n");
	}
	
	//	fprintf(stderr,"end of server loop\n");
	
	
	shutdown(sockfd, SHUT_RDWR);
	close(sockfd);
	
	close(connfd);
    }
    return NULL;
}


void msgStartServer(interfacesIntType *n, const int serverport) {
  fprintf(stderr,"**start** Stu's Autodiscover Tool (sat)  ->  port %d\n", serverport);

    int ip1,ip2,ip3,ip4;
    char *localhost = NULL;
    for (size_t ii = 0; ii < n->id; ii++) {
      if (strcmp(n->nics[ii]->devicename, "lo") == 0) {
	phyType *p = n->nics[ii];
	addrType ad = p->addr[0];
	sscanf(ad.addr,"%d.%d.%d.%d", &ip1,&ip2,&ip3,&ip4);
	localhost = strdup(ad.addr);
      }
    }
  
    pthread_t *pt;
    threadMsgType *tc;
    double *lasttime;
    numListType *nl;
    size_t num = 256 + 2;
    // 1..254 is the subnet, 256 is the server, 257 is the display

    //  CALLOC(gbps, num, sizeof(double));
    CALLOC(lasttime, num, sizeof(double));

    CALLOC(nl, num, sizeof(numListType));
    CALLOC(pt, num, sizeof(pthread_t));
    CALLOC(tc, num, sizeof(threadMsgType));

    for (size_t i = 0; i < num; i++) {
        nlInit(&nl[i], 1000);
    }

    for (size_t i = 0; i < num; i++) {
        tc[i].id = i;
        tc[i].num = num;
        tc[i].serverport = serverport;
	tc[i].n = n;
        //    tc[i].gbps = gbps;
	char s[20];
	sprintf(s, "%d.%d.%d.%zd", ip1, ip2, ip3, i);
        tc[i].tryhost = strdup(s);
        tc[i].localhost = localhost;
        tc[i].lasttime = lasttime;
        tc[i].starttime = timeAsDouble();
        tc[i].nl = nl;
        if (i==0) { // display
	  pthread_create(&(pt[i]), NULL, display, &(tc[i]));
	} else if (i ==1) {
	  pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
	} else if (i < 256) {
	  pthread_create(&(pt[i]), NULL, tryConnect, &(tc[i]));
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

  clusterIPs = calloc(100000, 1); assert(clusterIPs);
  memset(clusterIPs, 0, 100000);

  interfacesIntType *n = interfacesInit();
  interfacesScan(n);

  msgStartServer(n, 1600);

  free(clusterIPs);

  return 0;
}
