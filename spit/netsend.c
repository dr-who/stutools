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

void sendfile(FILE *fp, int sockfd);
ssize_t total=0;
int main(int argc, char* argv[])
{
    if (argc != 3) 
    {
        perror("usage:send_file  <IPaddress> <port>");
        exit(1);
    }

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) 
    {
        perror("Can't allocate sockfd");
        exit(1);
    }

    struct sockaddr_in serveraddr;
    memset(&serveraddr, 0, sizeof(serveraddr));
    serveraddr.sin_family = AF_INET;
    serveraddr.sin_port = htons(atoi(argv[2]));
    if (inet_pton(AF_INET, argv[1], &serveraddr.sin_addr) < 0)
    {
        perror("IPaddress Convert Error");
        exit(1);
    }

    if (connect(sockfd, (const struct sockaddr *) &serveraddr, sizeof(serveraddr)) < 0)
    {
        perror("Connect Error");
        exit(1);
    }
    
    char *buff = malloc(BUFFSIZE);
    assert(buff);

    ssize_t n;
    while ((n = send(sockfd, buff, BUFFSIZE, 0)) > 0) {
      //      fprintf(stderr,"sent %zd\n", n);
    }
    
    close(sockfd);
    return 0;
}

