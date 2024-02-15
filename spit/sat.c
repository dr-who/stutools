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

    while (keepRunning) {
        int serverport = tc->serverport;

        int sockfd = socket(AF_INET, SOCK_STREAM, 0);
        if (sockfd == -1) {
	  //            perror("Can't allocate sockfd");
	    continue;
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
	  continue;
        }

        if (listen(sockfd, serverport) == -1) {
	  perror("Listen Error");
	  close(sockfd);
	  exit(1);
	  continue;
        }

        socklen_t addrlen = sizeof(clientaddr);
        int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
	if (connfd == -1) {
	  perror("Connect Error");
	  exit(1);
	  close(sockfd);
	  continue;
	}

	//	socksetup(connfd, 10);
	    //        }
        char addr[INET_ADDRSTRLEN];
        inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN);

	char buffer[1024];
	if (sockrec(connfd, buffer, 1024, 0, 1) < 0) {
	  perror("sockrec connfd");
	  //	  fprintf(stderr, "din'yt get a string\n");
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
	  if (socksend(connfd, buffer, 0, 1) < 0) {
	    perror("socksendhello");
	  }
	} else if (strncmp(buffer,"interfaces",10)==0) {
	  char *json = interfacesDumpJSONString(tc->n);
	  if (socksend(connfd, json, 0, 1) < 0) {
	    perror("socksendinterfaces");
	  }
	  free(json);
	} else if (strncmp(buffer,"sum",3)==0) {
	  keyvalueType *kv = keyvalueInit();
	  size_t HDDsum = 0, SSDsum= 0, SSDsumGB = 0, HDDsumGB = 0;
	  size_t nodesGood = 0, nodesBad = 0;
	  size_t RAMsumGB = 0, Coressum = 0;
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    if (timeAsDouble() - tc->cluster->node[cc]->seen <= 11) {
	      nodesGood++;
	    } else {
	      nodesBad++;
	    }
	    HDDsum += tc->cluster->node[cc]->HDDcount;
	    SSDsum += tc->cluster->node[cc]->SSDcount;
	    HDDsumGB += tc->cluster->node[cc]->HDDsizeGB;
	    SSDsumGB += tc->cluster->node[cc]->SSDsizeGB;
	    RAMsumGB += tc->cluster->node[cc]->RAMGB;
	    Coressum += tc->cluster->node[cc]->Cores;
	  }
	  keyvalueSetLong(kv, "Coressum", Coressum);
	  keyvalueSetLong(kv, "RAMsumGB", RAMsumGB);
	  keyvalueSetLong(kv, "HDDsum", HDDsum);
	  keyvalueSetLong(kv, "SSDsum", SSDsum);
	  keyvalueSetLong(kv, "HDDsumGB", HDDsumGB);
	  keyvalueSetLong(kv, "SSDsumGB", SSDsumGB);
	  keyvalueSetLong(kv, "NodesGood", nodesGood);
	  keyvalueSetLong(kv, "NodesBad", nodesBad);
	    
	  char *json = keyvalueDumpAsJSON(kv);
	  if (socksend(connfd, json, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(json);
	  keyvalueFree(kv);
	} else if (strncmp(buffer,"bad",3)==0) {
	  keyvalueType *kv = keyvalueInit();
	  size_t nodesGood = 0, nodesBad = 0;
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    if (timeAsDouble() - tc->cluster->node[cc]->seen <= 11) {
	      nodesGood++;
	    } else {
	      nodesBad++;
	      char str[100];
	      sprintf(str, "badNode%02zd", nodesBad);
	      keyvalueSetString(kv, str, tc->cluster->node[cc]->nodename);
	      sprintf(str, "badNode%02zd_ip", nodesBad);
	      keyvalueSetString(kv, str, tc->cluster->node[cc]->ipaddress);
	    }
	  }
	  char *json = keyvalueDumpAsJSON(kv);
	  if (socksend(connfd, json, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(json);
	  keyvalueFree(kv);
	} else if (strncmp(buffer,"good",4)==0) {
	  keyvalueType *kv = keyvalueInit();
	  size_t nodesGood = 0, nodesBad = 0;
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    if (timeAsDouble() - tc->cluster->node[cc]->seen <= 11) {
	      nodesGood++;
	      char str[100];
	      sprintf(str, "goodNode%02zd", nodesGood);
	      keyvalueSetString(kv, str, tc->cluster->node[cc]->nodename);
	      sprintf(str, "goodNode%02zd_ip", nodesGood);
	      keyvalueSetString(kv, str, tc->cluster->node[cc]->ipaddress);
	    } else {
	      nodesBad++;
	    }
	  }
	  char *json = keyvalueDumpAsJSON(kv);
	  if (socksend(connfd, json, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(json);
	  keyvalueFree(kv);
	} else if (strncmp(buffer,"ansible",7)==0) {
	  char *str = calloc(1024*1024, 1); assert(str);
	  char *ret = str;
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    
	    str += sprintf(str, "%s ansible_ssh_host=%s\n", tc->cluster->node[cc]->nodename, tc->cluster->node[cc]->ipaddress);
	  }
	  fprintf(stderr,"%s", ret);
	  if (socksend(connfd, ret, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(ret);
	} else if (strncmp(buffer,"cluster",7)==0) {
	  //	  fprintf(stderr, "sending cluster info back: %zd nodes\n", tc->cluster->id);
	  char *json = clusterDumpJSONString(tc->cluster);
	  if (socksend(connfd, json, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(json);
	} else if (strncmp(buffer,"block", 5)==0) {
	  blockDevicesType *bd = blockDevicesInit();
	  
	  blockDevicesScan(bd);

	  for (size_t q = 0; q < bd->num; q++) {
	    //	  char *s = blockDevicesAllJSON(bd);
	    char *s = keyvalueDumpAsJSON(bd->devices[q].kv);
	    if (socksend(connfd, s, 0, 0) < 0) {
	      perror("socksendblock");
	    }
	    free(s);
	  }
	  
	  blockDevicesFree(bd);
	} else {
	  printf("?? %s\n", buffer);
	}
	
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
    clusterFree(&cluster); assert(cluster == NULL);
    free(tc);
    free(pt);
}






int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
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
