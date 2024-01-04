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


#include "sat.h"
#include "snack.h"
#include "simpmail.h"
#include "interfaces.h"
#include "cluster.h"
#include "iprange.h"
#include "ipcheck.h"


clusterType *cluster;



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
      fprintf(stderr,"*info* openfiles=%zd, cluster: %zd\n", np, cluster->id);
      if (np > 1000) {
	exit(-2);
      }
      char *json = clusterDumpJSONString(cluster);
      printf("%s", json);
      free(json);
      //      fprintf(stderr,"*display %d*\n", d->id);
      sleep(5);
    }
  }
  return NULL;
}


static void *client(void *arg) {
  threadMsgType *d = (threadMsgType*)arg;
  while (keepRunning) {

    char *ipaddress = d->tryhost;
    if (clusterFindNode(cluster, d->fqn) < 0) {
      sleep(1);
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

    int true = 1;
    if (setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &true, sizeof(int)) == -1) {
      //            perror("Setsockopt");
      close(sockfd);
      continue;
      //            exit(1);
    }

    //    if (socksetup(sockfd, 20) < 0) {
    //      perror("sock setup");
    //      close(sockfd);
    //      continue;
    //    }

    
    if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
      //      perror("Listening port not available");
      close(sockfd);
      continue;
    }

    char buff[1024];
    memset(buff, 0, 1024);
    sprintf(buff, "Hello %s %s\n", ipaddress, d->fqn);

    socksend(sockfd, buff, 0, 0);

    sockrec(sockfd, buff, 1024, 0, 1);
    if (strncmp(buff, "World!",6)==0) {
      char s1[100],ipa[100],fqn[100];
      sscanf(buff, "%s %s %s",s1, ipa, fqn); // world i'
      fprintf(stderr,"*info* client says it's a valid server = %s %s %s\n", s1,ipa,fqn);
      clusterAddNodesIP(cluster, d->fqn, ipaddress);
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
	  goto end;
	}
	
	if (strncmp(buffer,"Hello",5)==0) {
	  char s1[100],ipa[100],fqn[100];
	  printf("buffer: %s\n", buffer);
	  sscanf(buffer,"%s %s %s", s1,ipa,fqn);
	  fprintf(stderr,"*server says it's a welcome/valid client ... %s %s %s\n", s1, ipa, fqn);
	  sprintf(buffer,"World! %s %s", ipa, fqn);
	  if (socksend(connfd, buffer, 0, 1) < 0)
	    goto end;
	} else if (strncmp(buffer,"interfaces",10)==0) {
	  char *json = interfacesDumpJSONString(tc->n);
	  int err = socksend(connfd, json, 0, 1);
	  free(json);
	  if (err < 0) goto end;
	} else if (strncmp(buffer,"cluster",7)==0) {
	  char *json = clusterDumpJSONString(cluster);
	  socksend(connfd, json, 0, 1);
	  socksend(connfd, "\n", 0, 1);
	  free(json);
	} else {
	  sleep(1);
	  //	    fprintf(stderr,"*unknown handshake, invalid client\n");
	}
	
	//	fprintf(stderr,"end of server loop\n");
	

    end:
	  
	shutdown(sockfd, SHUT_RDWR);
	close(sockfd);
	
	close(connfd);
    }
    return NULL;
}


void msgStartServer(interfacesIntType *n, const int serverport) {
  fprintf(stderr,"**start** Stu's Autodiscover Tool (sat)  ->  port %d\n", serverport);

  // for each interface add all network ranges
    ipCheckType *ipcheck = ipCheckInit();

    // for each NIC
    for (size_t ii = 0; ii < n->id; ii++) {
      if (strcmp(n->nics[ii]->devicename, "lo") != 0) {
	phyType *p = n->nics[ii];
	for (size_t j = 0; j < p->num; j++) {
	  addrType ad = p->addr[j];
	  char cidr[30];
	  sprintf(cidr, "%s/%d", ad.broadcast, ad.cidrMask);

	  ipRangeType *ipr = ipRangeInit(cidr);
	  ipCheckAdd(ipcheck, n->nics[ii]->devicename, ipr->firstIP+1, ipr->lastIP-1);
	  ipRangeFree(ipr);
	}
      }
    }

    fprintf(stderr,"ips to examine: %zd\n", ipcheck->num);
    
    pthread_t *pt;
    threadMsgType *tc;
    double *lasttime;
    size_t num = ipcheck->num + 2;
    // 0 is server, 1 is display, 2 onwards are the IPs.

    //  CALLOC(gbps, num, sizeof(double));
    CALLOC(lasttime, num, sizeof(double));

    CALLOC(pt, num, sizeof(pthread_t));
    CALLOC(tc, num, sizeof(threadMsgType));


    for (size_t i = 0; i < num; i++) {
        tc[i].id = i;
        tc[i].num = num;
        tc[i].serverport = serverport;
	tc[i].n = n;
        tc[i].lasttime = lasttime;
        tc[i].starttime = timeAsDouble();
	if (i < ipcheck->num) {
	  tc[i].eth = ipcheck->interface[i];
	  char s[200];
	  unsigned int ip1 = 0, ip2 = 0, ip3 = 0, ip4 = 0;
	  ipRangeNtoA(ipcheck->ips[i].ip, &ip1, &ip2, &ip3, &ip4);
	  sprintf(s, "%d.%d.%d.%d", ip1, ip2, ip3, ip4);
	  tc[i].tryhost = strdup(s);
	  struct utsname buf;
	  uname(&buf);
	  sprintf(s, "%s-%s-%s-%zd", buf.nodename, tc[i].tryhost, tc[i].eth, interfaceSpeed(tc[i].eth));
	  tc[i].fqn = strdup(s);
	  pthread_create(&(pt[i]), NULL, client, &(tc[i]));
	} else if (i == ipcheck->num) {
	  pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
	} else {
	  pthread_create(&(pt[i]), NULL, display, &(tc[i]));
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

  int port = 1600;
  cluster = clusterInit(port);

  interfacesIntType *n = interfacesInit();
  interfacesScan(n);

  msgStartServer(n, port);

  return 0;
}
