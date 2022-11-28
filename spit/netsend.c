#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <libgen.h>
#include <unistd.h>
#include <arpa/inet.h>
#include <assert.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include "transfer.h"
#include "utils.h"

int keepRunning = 1;

void sendfile(FILE *fp, int sockfd);
ssize_t total=0;
int main(int argc, char* argv[])
{
    if (argc != 2) 
    {
        perror("usage:netsend  <IPaddress>");
        exit(1);
    }
    int port = 8877;

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) 
    {
        perror("Can't allocate sockfd");
        exit(1);
    }

    struct sockaddr_in serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    FILE *fp = fopen("/dev/random", "rb");
    long c = 0;
    if (fread(&c, sizeof(long), 1, fp) == 0) {
      perror("random");
    }
    fclose(fp);
    //    fprintf(stderr,"*random* %ld\n", c);
    srand(timedouble()*100000 + c);
    int tries = 0;
    do {
      port = 8877 + rand()%100;
      serveraddr.sin_family = AF_INET;
      serveraddr.sin_port = htons(port);
      if (inet_pton(AF_INET, argv[1], &serveraddr.sin_addr) < 0)
	{
	  perror("IPaddress Convert Error");
	  exit(1);
	}
      
      if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0)
	{
	  //	  perror("Listening port not available");
	  tries++;
	  if (tries > 1000) {
	    fprintf(stderr,"*error* couldn't find any ports. exiting\n");
	    exit(1);
	  }
	  continue;
	} else {
	break;
      }
      usleep(1);
    } while(1);
      
    //    fprintf(stdout,"*info* connected on port %d\n", port);
    
    char *buff = malloc(BUFFSIZE);
    assert(buff);

    ssize_t n;
    while ((n = send(sockfd, buff, BUFFSIZE, 0)) > 0) {
      //      fprintf(stderr,"sent %zd\n", n);
    }
    
    close(sockfd);
    return 0;
}

