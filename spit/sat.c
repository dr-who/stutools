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

#include "advertise-mc.h"
#include "respond-mc.h"
#include "echo.h"
#include "queue.h"

int keepRunning = 1;
int tty = 0;

volatile queueType *hostsToMonitor;

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

 
static void *client(void *arg) {
  if (arg) {};

  while (1) {
    sleep(1);
  }
  return NULL;
}


    
  


/*static void *scanner(void *arg) {
  if (arg) {}
  fprintf(stderr,"*scanner*\n");
  while (keepRunning) {
    ipCheckType *ipc = ipCheckInit();
    ipCheckAllInterfaceRanges(ipc);

    unsigned int ip = 0;
    while ((ip = ipCheckOpenPort(ipc, 1600, 0.05, 1)) != 0) {
      unsigned int ip1, ip2, ip3, ip4;
      ipRangeNtoA(ip, &ip1, &ip2, &ip3, &ip4);
      char s[20];
      sprintf(s,"%u.%u.%u.%u", ip1, ip2, ip3, ip4);
      queueAdd(hostsToMonitor, s);
      printf("adding to queue %s\n", s);
      sleep(10);
    }      
    
    //    ipCheckShowFound(ipc);
    ipCheckFree(ipc);
    sleep(60);
  }
  return NULL;
}

*/

static void *display(void *arg) {
  if (arg) {}
  //  threadMsgType *d = (threadMsgType*)arg;
  while (keepRunning) {
    sleep(1);
    /*    if (arg) {
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
      }*/
  }
  return NULL;
}

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
	  goto end;
	}
	
	if (strncmp(buffer,"Hello",5)==0) {
	  char s1[100],ipa[100];
	  printf("buffer: %s\n", buffer);
	  sscanf(buffer,"%*s %s %s", s1,ipa);
	  fprintf(stderr,"*server says it's a welcome/valid client ... %s %s\n", s1, ipa);

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

    pthread_t *pt;
    threadMsgType *tc;
    double *lasttime;
    const int extras = 4;
    // 4 extras. MC adv. MC watch. display and receive requests

    size_t num = 30; // threads
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
	if (i < num -extras) {
	  tc[i].eth = NULL; //ipcheck->interface[i];
	  /*
	  unsigned int ip1 = 0, ip2 = 0, ip3 = 0, ip4 = 0;
	  ipRangeNtoA(ipcheck->ips[i].ip, &ip1, &ip2, &ip3, &ip4);
	  sprintf(s, "%d.%d.%d.%d", ip1, ip2, ip3, ip4);
	  tc[i].tryhost = strdup(s);
	  */

	  struct utsname buf;
	  uname(&buf);
	  char s[200];
	  sprintf(s, "%s-%s-%s-%zd", buf.nodename, tc[i].tryhost, tc[i].eth, interfaceSpeed(tc[i].eth));
	  tc[i].fqn = strdup(s);
	  tc[i].tryhost = NULL;
	  pthread_create(&(pt[i]), NULL, client, &(tc[i]));
	} else if (i == num-extras) {
	  pthread_create(&(pt[i]), NULL, respondMC, &(tc[i]));
	} else if (i == num-3) {
	  pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
	} else if (i == num-2) {
	  pthread_create(&(pt[i]), NULL, advertiseMC, &(tc[i]));
	} else if (i == num-1) {
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

  hostsToMonitor = queueInit();

  int port = 1600;
  cluster = clusterInit(port);

  interfacesIntType *n = interfacesInit();
  interfacesScan(n);

  msgStartServer(n, port);

  return 0;
}
