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


int main(int argc, char *argv[]) {
    if (argc < 2) {
        perror("usage: snacksend <IPaddress>");
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

    //    fprintf(stdout,"*info* connected on port %d\n", port);

    char *buff = malloc(BUFFSIZE);
    assert(buff);

    ssize_t n;
    clock_t lastclock = clock();
    double lasttime = time(NULL);
    while ((n = send(sockfd, buff, BUFFSIZE, 0)) > 0) {
        if (argc > 2) {
            if (time(NULL) - lasttime > 1) {
                fprintf(stdout, "*info* [port %d] CPU %.1lf %% (100%% is one core)\n", port,
                        (clock() - lastclock) * 100.0 / (time(NULL) - lasttime) / CLOCKS_PER_SEC);
                lasttime = time(NULL);
                lastclock = clock();
            }
        }
        //      fprintf(stderr,"sent %zd\n", n);
    }

    close(sockfd);
    return 0;
}

