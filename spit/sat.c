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
#include "multicast.h"

#include "blockdevice.h"
#include "json.h"
#include "nfsexports.h"

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

 
void *httpd(void *arg) {
    threadMsgType *tc = (threadMsgType *) arg;

      int serverport = 8080;

        int sockfd = socket(AF_INET, SOCK_STREAM, 0);
        if (sockfd == -1) {
	  perror("Can't allocate sockfd");
	  exit(1);
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

	while (keepRunning) {
	  
        if (listen(sockfd, serverport) == -1) {
	  perror("Listen Error");
	  close(sockfd);
	  exit(1);
	  //	  continue;
        }


	socklen_t addrlen = sizeof(clientaddr);
	  int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
	  if (connfd == -1) {
	    perror("Connect Error");
	    exit(1);
	    close(sockfd);
	    //	    continue;
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
	  
	  //	  fprintf(stderr, "received:%s\n", buffer);


	  char outstr[200];
	  time_t t;
	  struct tm *tmp;
	  
	  t = time(NULL);
	  tmp = gmtime(&t);
	  if (tmp == NULL) {perror("localtime"); exit(EXIT_FAILURE); }
	  // Date: Thu, 22 Feb 2024 06:43:59 GMT
	  //Date: Thu, 22 Feb 2024 06:48:16 GMT
	    
	  strftime(outstr, sizeof(outstr), "%a, %d %b %Y %H:%M:%S GMT", tmp);
	  
	  char *con = clusterDumpJSONString(tc->cluster);
	  sprintf(buffer,"HTTP/1.1 200 OK\nCache-Control: no-cache\nDate: %s\nConnection: close\nContent-Type: text/plain\nContent-Length: %zd\n\n", outstr, strlen(con));
	  
	  assert(strlen(buffer) < 1023);
	  
	  char *reply = malloc(strlen(con) + strlen(buffer) + 100); assert(reply);
	  sprintf(reply, "%s%s", buffer, con);


	  if (socksend(connfd, reply, 0, 1) < 0) {
	    perror("socksend\n");
	  }
	  

	  shutdown(connfd,  SHUT_RDWR);
	  close(connfd);

	  free(reply);
	  free(con);

	}
	close(sockfd);
    return NULL;
}


void *receiver(void *arg) {
    threadMsgType *tc = (threadMsgType *) arg;

        int serverport = tc->serverport;

        int sockfd = socket(AF_INET, SOCK_STREAM, 0);
        if (sockfd == -1) {
	  perror("Can't allocate sockfd");
	  exit(1);
	  //	    continue;
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

        if (listen(sockfd, serverport) == -1) {
	  perror("Listen Error");
	  close(sockfd);
	  exit(1);
	  //	  continue;
        }

	    while (keepRunning) {


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

	fprintf(stderr, "received:%s\n", buffer);
	
	if (strncmp(buffer,"/api",4)==0) {
	} else if (strncmp(buffer,"Hello",5)==0) {
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
	  char *bad = clusterGoodBad(tc->cluster, &nodesGood, &nodesBad);
	  free(bad);
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    HDDsum += keyvalueGetLong(tc->cluster->node[cc]->info, "HDDcount");
	    SSDsum += keyvalueGetLong(tc->cluster->node[cc]->info, "SSDcount");

	    HDDsumGB += keyvalueGetLong(tc->cluster->node[cc]->info, "HDDsizeGB");
	    SSDsumGB += keyvalueGetLong(tc->cluster->node[cc]->info, "SSDsizeGB");

	    RAMsumGB += keyvalueGetLong(tc->cluster->node[cc]->info, "RAMGB");
	    Coressum += keyvalueGetLong(tc->cluster->node[cc]->info, "Cores");
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
	      keyvalueSetString(kv, str, tc->cluster->node[cc]->name);
	      sprintf(str, "badNode%02zd_ip", nodesBad);
	      keyvalueSetString(kv, str, keyvalueGetString(tc->cluster->node[cc]->info, "ip"));
	      sprintf(str, "badNode%02zd_lastSeen", nodesBad);
	      keyvalueSetLong(kv, str, timeAsDouble() - tc->cluster->node[cc]->seen);
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
	      keyvalueSetString(kv, str, tc->cluster->node[cc]->name);
	      sprintf(str, "goodNode%02zd_ip", nodesGood);
	      keyvalueSetString(kv, str, keyvalueGetString(tc->cluster->node[cc]->info, "ip"));
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
	    char *nn = keyvalueGetString(tc->cluster->node[cc]->info, "nodename");
	    char *ii = keyvalueGetString(tc->cluster->node[cc]->info, "ip");
	    str += sprintf(str, "%s ansible_ssh_host=%s\n", nn, ii);
	    free(nn);
	    free(ii);
	  }
	  fprintf(stderr,"%s", ret);
	  if (socksend(connfd, ret, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(ret);
	} else if (strncmp(buffer,"hosts",5)==0) {
	  char *str = calloc(1024*1024, 1); assert(str);
	  char *ret = str;
	  str += sprintf(str, "127.0.0.1   localhost localhost.localdomain localhost4 localhost4.localdomain4\n");
	  str += sprintf(str, "::1         localhost localhost.localdomain localhost6 localhost6.localdomain6\n\n");
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    
	    char *nn = keyvalueGetString(tc->cluster->node[cc]->info, "nodename");
	    char *ii = keyvalueGetString(tc->cluster->node[cc]->info, "ip");
	    str += sprintf(str, "%s\tnode%d %s\n", ii, cc, nn);
	    free(nn);
	    free(ii);
	  }
	  fprintf(stderr,"%s", ret);
	  if (socksend(connfd, ret, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(ret);
	} else if (strncmp(buffer,"fstab",5)==0) {
	  char *str = calloc(1024*1024, 1); assert(str);
	  char *ret = str;
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    for (size_t i = 0; i < 64; i++) {
	      str += sprintf(str, "node%d:/mnt/disk%zd\t/mnt/node%d-%zd\tnfs\tdefaults,noauto\t 0 0\n", cc, i, cc,i);
	    }
	  }
	  fprintf(stderr,"%s", ret);
	  if (socksend(connfd, ret, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(ret);
	} else if (strncmp(buffer,"mkdir",5)==0) {
	  char *str = calloc(1024*1024, 1); assert(str);
	  char *ret = str;
	  for (int cc = 0; cc < tc->cluster->id; cc++) {
	    for (size_t i = 0; i < 64; i++) {
	      str += sprintf(str, "mkdir -p /mnt/node%d-%zd\n", cc, i);
	    }
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
	} else if (strncmp(buffer,"nfs", 3)==0) {
	  nfsRangeExports *n = nfsExportsInit();
	  
	  jsonValue* json = nfsExportsJSON(n);
	  char *ss = jsonValueDump(json);
	  if (socksend(connfd, ss, 0, 1) < 0) {
	    perror("socksendcluster");
	  }
	  free(json);
	  
	  nfsExportsFree(&n);
	  free(n);
	} else if (strncmp(buffer,"block", 5)==0) {
	  jsonValue *j = bdScan();
	  
	  char * s = jsonValueDump(j);
	  if (socksend(connfd, s, 0, 0) < 0) {
	    perror("socksendblock");
	  }
	  free(s);
	} else {
	  printf("?? %s\n", buffer);
	}
	
	close(connfd);
	    }
	//	close(sockfd);
    return NULL;
}


void startThreads(interfacesIntType *n, const int serverport) {
  fprintf(stderr,"**start** Stu's Autodiscover Tool (sat)  ->  port %d\n", serverport);

    pthread_t *pt;
    threadMsgType *tc;
    size_t num = 4; // threads

    CALLOC(pt, num, sizeof(pthread_t));
    CALLOC(tc, num, sizeof(threadMsgType));

    clusterType *cluster = clusterInit(serverport);
    loadEnvVars("/etc/sat.cfg");
    clusterSetAlertEmail(cluster, getenv("ALERT_TOEMAIL"), getenv("ALERT_FROMEMAIL"), getenv("ALERT_FROMNAME"), getenv("ALERT_SUBJECT"));

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
	} else if (i == 3) {
	  pthread_create(&(pt[i]), NULL, httpd, &(tc[i]));
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

  size_t port = clusterDefaultPort();
  startThreads(n, port);

  return 0;
}
