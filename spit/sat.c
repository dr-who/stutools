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

static void *display(void *arg) {
  //  threadMsgType *d = (threadMsgType*)arg;
  while (keepRunning) {
    if (arg) {
      fprintf(stderr,"*info* display: %s\n", clusterIPs);
      
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
      continue;
    }

    if (socksetup(sockfd, 20) < 0) {
      perror("sock setup");
      close(sockfd);
      continue;
    }

    
    if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
      //      perror("Listening port not available");
      continue;
    }

    char buff[1024];
    sprintf(buff, "%s", "HELLO WORLD\n");

    socksend(sockfd, buff, 0, 0);

    fprintf(stderr,"*info* waiting for reply\n");

    sockrec(sockfd, buff, 1024, 0, 0);
    if (strcmp(buff, "!\n")==0) {
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

	int validclient = 0;
	char buffer[1024];
	sockrec(connfd, buffer, 1024, 0, 0);

	if (strcmp(buffer,"HELLO WORLD\n")==0) {
	  validclient = 1;
	}

	if (validclient) {
	  fprintf(stderr,"*server says it's a welcome/valid client = %s\n", addr);
	} else {
	  fprintf(stderr,"*no handshake, invalid client\n");
	}

	sprintf(buffer,"!\n");
	socksend(connfd, buffer, 0, 0);

        shutdown(sockfd, SHUT_RDWR);
        close(sockfd);

        close(connfd);
    }
    return NULL;
}


void msgStartServer(const int ip1, const int ip2, const int ip3, const int ip4, const char *localserver, const int serverport) {
  fprintf(stderr,"**start** Stu's Autodiscover Tool (sat) %d.%d.%d.%d/%s   ->  port %d\n", ip1,ip2,ip3,ip4,localserver, serverport);
  
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
        //    tc[i].gbps = gbps;
	char s[20];
	sprintf(s, "%d.%d.%d.%zd", ip1, ip2, ip3, i);
        tc[i].tryhost = strdup(s);
        tc[i].localhost = strdup(localserver);
        tc[i].lasttime = lasttime;
        tc[i].starttime = timeAsDouble();
        tc[i].nl = nl;
	if (i < 256) {
	  pthread_create(&(pt[i]), NULL, tryConnect, &(tc[i]));
	} else if (i == 257) {
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

  memset(clusterIPs, 0, 10000);
  clusterIPs = calloc(100000, 1); assert(clusterIPs);

		/*  size_t numDevices;
  stringType *devs = listDevices(&numDevices);
  for (size_t i = 0; i < numDevices; i++) {
    fprintf(stderr,"*%zd*:  %s\n", i, devs->path);
    getEthStats(devs, numDevices);
    }*/


  int ip1 = 0, ip2 = 0, ip3 = 0, ip4 = 0;


    struct ifaddrs *ifaddr;
    int family;
    char host[NI_MAXHOST];

    if (getifaddrs(&ifaddr) == -1) {
        perror("getifaddrs");
        exit(EXIT_FAILURE);
    }

    printf("if    \tIPv4         \tHW/MAC           \tLINK  \tSPEED\tMTU\tChanges\n");

    for (struct ifaddrs *ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
        if (ifa->ifa_addr == NULL)
            continue;

        family = ifa->ifa_addr->sa_family;

        /* Display interface name and family (including symbolic
           form of the latter for the common families). */

        if (family == AF_INET/* || family == AF_INET6*/) {
	  if (strcmp(ifa->ifa_name, "lo")==0) {
	    continue; // don't print info on localhost
	  }
	  
	  printf("%s\t", ifa->ifa_name);

	  int s = getnameinfo(ifa->ifa_addr,
                            (family == AF_INET) ? sizeof(struct sockaddr_in) :
                            sizeof(struct sockaddr_in6),
                            host, NI_MAXHOST,
                            NULL, 0, NI_NUMERICHOST);
            if (s != 0) {
                printf("getnameinfo() failed: %s\n", gai_strerror(s));
                exit(EXIT_FAILURE);
            }

	    ip1=ifa->ifa_addr->sa_data[2];
	    ip2=ifa->ifa_addr->sa_data[3];
	    ip3=ifa->ifa_addr->sa_data[4];
	    ip4=ifa->ifa_addr->sa_data[5];
            printf("%s\t", host);
            cmd_printHWAddr(ifa->ifa_name); printf("\t");

            char ss[PATH_MAX];
            sprintf(ss, "/sys/class/net/%s/carrier", ifa->ifa_name);
	    //            dumpFile(ss, "", 0);
	    double linkup = getValueFromFile(ss, 1);
	    printf("%-6s", linkup>=1 ? "UP" : "DOWN");
	    printf("\t");

            sprintf(ss, "/sys/class/net/%s/speed", ifa->ifa_name);
	    double speed = getValueFromFile(ss, 1);
	    printf("%.0lf\t", speed);

            sprintf(ss, "/sys/class/net/%s/mtu", ifa->ifa_name);
	    double mtu = getValueFromFile(ss, 1);
	    printf("%.0lf\t", mtu);

            sprintf(ss, "/sys/class/net/%s/carrier_changes", ifa->ifa_name);
	    double cc = getValueFromFile(ss, 1);
	    printf("%.0lf\n", cc);
        }
    }

    freeifaddrs(ifaddr);
  
    msgStartServer(ip1, ip2, ip3, ip4, host, 9200);

    free(clusterIPs);

  return 0;
}
