#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include "transfer.h"

#include "utils.h"
#include "numList.h"

int keepRunning = 1;

typedef struct {
  int id;
  int num;
  int serverport;
  double *gbps;
  double *lasttime;
  numListType nl;
  char **ips;
} threadInfoType;

static void *receiver(void *arg) 
{
  threadInfoType *tc = (threadInfoType*)arg;
  //  fprintf(stderr,"%d\n", tc->serverport);
  while (keepRunning) {

    int serverport = tc->serverport;
  
    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd == -1) 
      {
        perror("Can't allocate sockfd");
        exit(1);
      }

    int true = 1;
    if (setsockopt(sockfd,SOL_SOCKET,SO_REUSEADDR,&true,sizeof(int)) == -1)
      {
	perror("Setsockopt");
	exit(1);
      }

    
    struct sockaddr_in clientaddr, serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    serveraddr.sin_family = AF_INET;
    serveraddr.sin_addr.s_addr = htonl(INADDR_ANY);
    serveraddr.sin_port = htons(serverport);

    if (bind(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) == -1) 
      {
        perror("Bind Error");
        exit(1);
      }

    if (listen(sockfd, 7788+tc->id) == -1) {
      perror("Listen Error");
      exit(1);
    }

    socklen_t addrlen = sizeof(clientaddr);
    int connfd = accept(sockfd, (struct sockaddr *) &clientaddr, &addrlen);
    if (connfd == -1) 
      {
        perror("Connect Error");
        exit(1);
      }
    char addr[INET_ADDRSTRLEN];
    inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN);
    tc->ips[tc->id] = strdup(addr);
    shutdown(sockfd, SHUT_RDWR);
    close(sockfd); 

    printf("Start receive file: from %s\n", addr);//inet_ntop(AF_INET, &clientaddr.sin_addr, addr, INET_ADDRSTRLEN));
    //    writefile(connfd, fp);



    /*
    // where socketfd is the socket you want to make non-blocking
    int status = fcntl(connfd, F_SETFL, fcntl(connfd, F_GETFL, 0) | O_NONBLOCK);

    if (status == -1){
      perror("calling fcntl");
      // handle the error.  By the way, I've never seen fcntl fail in this way
    }
    */
    
    char buff[MAX_LINE] = {0};
    ssize_t n;
    
    double lasttime = timedouble();
    ssize_t total = 0;
    
    double globaltime = 0;
    size_t globalbytes = 0;
    while ((n = recv(connfd, buff, MAX_LINE, 0)) > 0) 
      {
	double thistime = timedouble();
	tc->lasttime[tc->id] = thistime;
	if (thistime < lasttime + 5) {
	  globaltime += (thistime - lasttime);
	  globalbytes += n;
	} else {
	  globalbytes = 0;
	  globaltime = 0;
	}
	lasttime = thistime;
      
	total+=n;

	const double gbps = TOGB(globalbytes) * 8 / (globaltime);
	tc->gbps[tc->id] = gbps;
	nlAdd(&tc->nl, gbps);
      
	//	fprintf(stdout,"[%d] %zd, time %.4lf,  speed %.3lf Gb/s\n", tc->id, total, globaltime/tc->num, nlMean(&tc->nl));

	if (n == -1)
	  {
	    perror("Receive File Error");
	    exit(1);
	  }
      
	//	printf("Receive Success, NumBytes = %ld\n", total);
      }

    close(connfd);
  }
  return NULL;
}


void *display(void *arg) {
  threadInfoType *tc = (threadInfoType*)arg;
  while(1) {
    double t = 0;
    for (int i = 0; i < tc->num;i++) {
      if (timedouble() - tc->lasttime[i] > 2) {
	tc->gbps[i] = 0;
      }
      fprintf(stdout, "[%d - %s], %.1lf Gb/s\n", SERVERPORT+i, tc->ips[i] ? tc->ips[i] : "", tc->gbps[i]);
      t += tc->gbps[i];
    }
    fprintf(stdout, "--> total %.2lf Gb/s\n", t);
    sleep(1);
  }
}

void startServers(size_t num) {
  pthread_t *pt;
  threadInfoType *tc;
  double *gbps, *lasttime;
  char **ips;
  CALLOC(gbps, num, sizeof(double));
  CALLOC(ips, num, sizeof(char*));
  CALLOC(lasttime, num, sizeof(double));

  CALLOC(pt, num+1, sizeof(pthread_t));
  CALLOC(tc, num+1, sizeof(threadInfoType));

  for (size_t i = 0; i < num; i++) {
    tc[i].id = i;
    tc[i].num = num;
    tc[i].serverport = SERVERPORT + i;
    tc[i].gbps = gbps;
    tc[i].ips = ips;
    tc[i].lasttime = lasttime;
    nlInit(&tc[i].nl, 10);
    pthread_create(&(pt[i]), NULL, receiver, &(tc[i]));
  }

  tc[num].id=num;
  tc[num].num=num;
  tc[num].serverport = 0;
  tc[num].gbps = gbps;
  tc[num].ips = ips;
  tc[num].lasttime = lasttime;
  pthread_create(&(pt[num]), NULL, display, &(tc[num]));

  for (size_t i = 0; i < num+1; i++) {
    pthread_join(pt[i], NULL);
  }
  free(tc);
  free(pt);
}



int main(int argc, char *argv[]) {

  // args

  if (argc != 2) {
    fprintf(stderr,"*info* usage ./netrec threads\n");
    exit(1);
  }
  // start servers
  startServers(atoi(argv[1]));
  

  return 0;
}
