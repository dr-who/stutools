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
#include "transfer.h"

#include "utils.h"
#include "numList.h"

#include <glob.h>

int keepRunning = 1;
int tty = 0;

#include "msgs.h"
#include "snack.h"

static void *display(void *arg) {
  threadMsgType *d = (threadMsgType*)arg;
  while (keepRunning) {
    if (arg) {
      fprintf(stderr,"*display %d\n", d->id);
      sleep(1);
    }
  }
  return NULL;
}


static void *tryConnect(void *arg) {
  if (arg) {
    fprintf(stderr,"*try connect ip *\n");
    sleep(1);
  }
  return NULL;
}



static void *receiver(void *arg) {
  fprintf(stderr,"*receiver!\n");
    threadMsgType *tc = (threadMsgType *) arg;
    while (keepRunning) {
        int serverport = tc->serverport;

        int sockfd = socket(AF_INET, SOCK_STREAM, 0);
        if (sockfd == -1) {
            perror("Can't allocate sockfd");
	    continue;
        }

        int true = 1;
        if (setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &true, sizeof(int)) == -1) {
            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
	struct timeval timeout;
	timeout.tv_sec = 5;
	timeout.tv_usec = 0;
        if (setsockopt(sockfd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)) == -1) {
            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
        if (setsockopt(sockfd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout)) == -1) {
            perror("Setsockopt");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }
	

        struct sockaddr_in clientaddr, serveraddr;
        memset(&serveraddr, 0, sizeof(serveraddr));
        serveraddr.sin_family = AF_INET;
        serveraddr.sin_addr.s_addr = htonl(INADDR_ANY);
        serveraddr.sin_port = htons(serverport);

        if (bind(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) == -1) {
            perror("Bind Error");
	  close(sockfd);
	  continue;
	    //            exit(1);
        }

        if (listen(sockfd, 7788 + tc->id) == -1) {
            perror("Listen Error");
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
        tc->ips[tc->id] = strdup(addr);
        shutdown(sockfd, SHUT_RDWR);
        close(sockfd);

        printf("New connection from %s\n", addr);
        // reset stats
        for (int i = 0; i < tc->num; i++) {
            nlShrink(&tc->nl[i], 10);
            if (i == tc->id) {
                nlClear(&tc->nl[i]); // this one is zero
            }
        }
        //    printf("Cleared\n");

        char *buff = malloc(BUFFSIZE);
        assert(buff);
        ssize_t n;

        double lasttime = timeAsDouble();

        double sumTime = 0;
        size_t sumBytes = 0;
	
        while (keepRunning && (n = recv(connfd, buff, BUFFSIZE, 0)) > 0) {
            const double thistime = timeAsDouble();
            tc->lasttime[tc->id] = thistime;
            const double gaptime = thistime - lasttime;
            lasttime = thistime;

            sumTime += gaptime;
            sumBytes += n;

            if (sumTime > 0.01) { // every 1/100 sec then add a data point
                const double gbps = TOGB(sumBytes) * 8 / (sumTime);
                //	tc->gbps[tc->id] = gbps;
                nlAdd(&tc->nl[tc->id], gbps);

                sumTime = 0;
                sumBytes = 0;
            }

            if (n == -1) {
                perror("Receive File Error");
		continue;
		//                exit(1);
            }
        }

        close(connfd);
    }
    return NULL;
}


void msgStartServer(const int serverport) {
  fprintf(stderr,"*msgstartserver %d\n", serverport);
  
    pthread_t *pt;
    threadMsgType *tc;
    double *lasttime;
    char **ips;
    numListType *nl;
    size_t num = 258;
    // 1..254 is the subnet, 256 is the server, 257 is the display

    //  CALLOC(gbps, num, sizeof(double));
    CALLOC(ips, num, sizeof(char *));
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
        tc[i].ips = ips;
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





///sending

void msgClient(const char *ipaddress, size_t bufSizeToUse, const int serverport, const size_t threads) {
  assert(ipaddress);
  assert(bufSizeToUse);
  assert(threads >= 1);
  
#ifdef __WIN32
    WSADATA wsaData;
    WSAStartup(MAKEWORD(2, 2), &wsaData);
#endif

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        perror("Can't allocate sockfd");
        return;
    }

    struct sockaddr_in serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    long c = getDevRandomLong();
    srand(c);
    
    int tries = 0, port = 0;
    do {
        port = serverport + rand() % threads;
        serveraddr.sin_family = AF_INET;
        serveraddr.sin_port = htons(port);

        if (inet_aton(ipaddress, &serveraddr.sin_addr) < 0)
            //      if (inet_pton(AF_INET, argv[1], &serveraddr.sin_addr) < 0)
        {
            perror("IPaddress Convert Error");
            return;
        }

        if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
            //	  perror("Listening port not available");
            tries++;
            if (tries > 1000) {
                fprintf(stderr, "*error* couldn't find any ports. exiting\n");
		return;
            }
            continue;
        } else {
            break;
        }
        usleep(1);
    } while (keepRunning);

    fprintf(stderr,"*info* connected to %s on port %d, size = %zd\n", ipaddress, port, bufSizeToUse);

    char *buff = aligned_alloc(4096, bufSizeToUse);
    assert(buff);

    ssize_t n;
    clock_t lastclock = clock();
    double lasttime = time(NULL);
    size_t lastcount = 0, thiscount = 0;
    
    while (keepRunning && ((n = send(sockfd, buff, bufSizeToUse, 0)) > 0)) {
      if ((size_t)n==bufSizeToUse) {
	thiscount++;
      }
      clock_t thistime = time(NULL);
      if (thistime - lasttime > 1) {
	const clock_t thisclock = clock();
	fprintf(stdout, "*info* [port %d] CPU %.1lf %% (100%% is one core), %zd IOPS, %.1lf µs latency\n", port,
		(thisclock - lastclock) * 100.0 / (thistime - lasttime) / CLOCKS_PER_SEC, (thiscount - lastcount), 1000000.0/(thiscount - lastcount));
	lasttime = thistime;
	lastclock = thisclock;
	lastcount = thiscount;
      }
    }

    free(buff);

    close(sockfd);
    return;
}


int main() {
  size_t numDevices;
  stringType *devs = listDevices(&numDevices);
  for (size_t i = 0; i < numDevices; i++) {
    fprintf(stderr,"*%zd*:  %s\n", i, devs->path);
    getEthStats(devs, numDevices);
  }


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
  
  exit(0);
  
  msgStartServer(9200);

  return 0;
}
