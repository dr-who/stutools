#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <libgen.h>
#include <unistd.h>
#include <assert.h>
#include <time.h>
#include "transfer.h"

#if defined(__linux__) || defined(__APPLE__)

#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>

#endif

#ifdef __WIN32
#include <winsock.h>
int inet_aton(const char *cp, struct in_addr *addr)
{
  addr->s_addr = inet_addr(cp);
  return (addr->s_addr == INADDR_NONE) ? 0 : 1;
}
#endif

#define MIN(x, y) (((x) < (y)) ? (x) : (y))

int main(int argc, char *argv[]) {
    if (argc < 2) {
        perror("usage: snacksend <IPaddress> <size>");
        exit(1);
    }
    int port = 8877;

#ifdef __WIN32
    WSADATA wsaData;
    WSAStartup(MAKEWORD(2, 2), &wsaData);
#endif

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        perror("Can't allocate sockfd");
        exit(1);
    }

    struct sockaddr_in serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    FILE *fp = fopen("/dev/random", "rb");
    long c = 0;
    if (fread(&c, sizeof(long), 1, fp) == 0) {
        //      perror("random");
        c = time(NULL);
        fprintf(stderr, "*info* random seed from clock\n");
    }
    fclose(fp);
    //    fprintf(stderr,"*random* %ld\n", c);
    srand(c);
    int tries = 0;
    do {
        port = 8877 + rand() % 100;
        serveraddr.sin_family = AF_INET;
        serveraddr.sin_port = htons(port);

        if (inet_aton(argv[1], &serveraddr.sin_addr) < 0)
            //      if (inet_pton(AF_INET, argv[1], &serveraddr.sin_addr) < 0)
        {
            perror("IPaddress Convert Error");
            exit(1);
        }

        if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0) {
            //	  perror("Listening port not available");
            tries++;
            if (tries > 1000) {
                fprintf(stderr, "*error* couldn't find any ports. exiting\n");
                exit(1);
            }
            continue;
        } else {
            break;
        }
        usleep(1);
    } while (1);

    size_t bufSizeToUse = BUFFSIZE;
    if (argc > 2) {
      bufSizeToUse = atoi(argv[2]);
    }
    
    fprintf(stderr,"*info* connected to %s on port %d, size = %zd\n", argv[1], port, bufSizeToUse);

    
    char *buff = aligned_alloc(4096, bufSizeToUse);
    assert(buff);

    ssize_t n;
    clock_t lastclock = clock();
    double lasttime = time(NULL);
    size_t lastcount = 0, thiscount = 0;
    
    while ((n = send(sockfd, buff, bufSizeToUse, 0)) > 0) {
      if ((size_t)n==bufSizeToUse) {
	thiscount++;
      }
        if (argc > 2) {
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
        //      fprintf(stderr,"sent %zd\n", n);
    }

    free(buff);

    close(sockfd);
    return 0;
}

