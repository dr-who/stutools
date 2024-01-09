// grabbed bits of this off Stack Overflow

#include <stdlib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <arpa/inet.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>


int sockconnect(const char *ipaddress, const size_t port, const double time_out){
   int sock = socket(AF_INET, SOCK_STREAM, 0);

   const int enable = 1;
   if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &enable, sizeof(int)) < 0)
     perror("setsockopt(SO_REUSEADDR) failed");
   if (setsockopt(sock, SOL_SOCKET, SO_REUSEPORT, &enable, sizeof(int)) < 0)
     perror("setsockopt(SO_REUSEADDR) failed");

   
   struct timeval timeout;      
   timeout.tv_sec = (int)time_out;
   timeout.tv_usec = (time_out - (int)time_out) * 1000000;
   
   if (setsockopt (sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof (timeout)) < 0) {
     //     perror("setsockopt failed\n");
     return -1;
   }
   
   if (setsockopt (sock, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof (timeout)) < 0) {
     //     perror("setsockopt failed\n");
     return -1;
   }

   struct sockaddr_in serv_addr;
   memset(&serv_addr, 0, sizeof(serv_addr));
   serv_addr.sin_port = htons(port);
   serv_addr.sin_family = AF_INET;

   if(inet_pton(AF_INET, ipaddress, &serv_addr.sin_addr) > 0){
     int ret = connect(sock, (struct sockaddr*)&serv_addr, sizeof(serv_addr));
     if (ret != 0) {
       //   perror(ipaddress);
       return -1;
     }
   } else{
     //     perror(ipaddress);
     //     fprintf(stderr,"*error* problem connecting to %s port 25\n", ipaddress);
     return -1;
   }
   return sock;
}

int socksetup(int fd, const int timeout_seconds) {
  struct timeval timeout;
  timeout.tv_sec = timeout_seconds;
  timeout.tv_usec = 0;
  if (setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)) == -1) {
    //    perror("Setsockopt");
    return -1;
  }
  if (setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout)) == -1) {
    //    perror("Setsockopt");
    return -1;
  }
  return 0;
}


int socksend(int fd, char *s, int flags, const int quiet) {
  int ret = send(fd, s, strlen(s), flags);
  if (quiet == 0) fprintf(stderr,"C[%d]: %s", ret, s);
  return ret;
}

int sockrec(int fd, char *buffer, int len, int flags, const int quiet) {
  int ret = recv(fd, buffer, len, flags);
  if (ret >= 0) {
    buffer[ret] = 0; // make string zero terminated
    if (quiet == 0) printf("S[%d]: %s", ret, buffer);
  }
  return ret;
}



int sockclose(int fd) {
  shutdown(fd, SHUT_RDWR);
  int r = close(fd);
  return r;
}
