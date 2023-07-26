// grabbed bits of this off Stack Overflow

#include <stdlib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <string.h>
#include <time.h>
#include <arpa/inet.h>

int simpmailConnect(const char *IPADDRESS){
   int sock = socket(AF_INET, SOCK_STREAM, 0);

   setsockopt(sock, SOL_SOCKET, SO_REUSEADDR | SO_REUSEPORT, NULL, 0);

   struct sockaddr_in serv_addr;
   memset(&serv_addr, 0, sizeof(serv_addr));
   serv_addr.sin_port = htons(25);
   serv_addr.sin_family = AF_INET;

   if(inet_pton(AF_INET, IPADDRESS, &serv_addr.sin_addr) > 0){
     int ret = connect(sock, (struct sockaddr*)&serv_addr, sizeof(serv_addr));
     if (ret != 0) {
       perror(IPADDRESS);
     }
   } else{
     perror(IPADDRESS);
     //     fprintf(stderr,"*error* problem connecting to %s port 25\n", IPADDRESS);
     return -1;
   }
   return sock;
}

void simpmailClose(int fd) {
  shutdown(fd, 0);
}



void thesend(int fd, char *s, int flags) {
  send(fd, s, strlen(s), flags);
  fprintf(stderr,"C: %s", s);
}

void therec(int fd, char *buffer, int len, int flags) {
  memset(buffer, 0, 1024);
  recv(fd, buffer, len, flags);
  printf("S: %s", buffer);
}


void simpmailSend(int fd, char *from, char *to, char *subject, char *body) {
  if (fd < 0) {
    fprintf(stderr,"*error* no fd, not sending email\n");
    return;
  }

   char buffer[1024];

   memset(buffer, 0, sizeof(buffer));
   int FLAGS = 0;

   // get response from Server after connection
   therec(fd, buffer, 1024, FLAGS);

   // get response from Server after HELO (program blocks here)
   thesend(fd, "HELO 127.0.0.1\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   sprintf(buffer,"MAIL FROM: <%s>\r\n", from);
   thesend(fd, buffer, FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   sprintf(buffer,"RCPT TO: <%s>\r\n", to);
   thesend(fd, buffer, FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   thesend(fd, "DATA\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   long ll = time(NULL);
   char timestring[1000];
   struct tm tm = *localtime(&ll);
   strftime(timestring, 999, "%a, %d %b %Y %H:%M:%S %z (%Z)", &tm);

   sprintf(buffer,"Date: %s\r\n", timestring);
   thesend(fd, buffer, FLAGS);

   sprintf(buffer,"Return-Path: <%s>\r\n", from);
   thesend(fd, buffer, FLAGS);
   
   sprintf(buffer, "From: <%s>\r\n", from);
   thesend(fd, buffer, FLAGS);

   sprintf(buffer, "To: <%s>\r\n", to);
   thesend(fd, buffer, FLAGS);

   sprintf(buffer, "Subject: %s\r\n", subject);
   thesend(fd, buffer, FLAGS);

   thesend(fd, "Content-Type: text/html\r\n", FLAGS);
   
   thesend(fd, "\r\n", FLAGS);
   thesend(fd, body, FLAGS);
   thesend(fd, "\r\n", FLAGS);
   thesend(fd, ".\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);
   
   thesend(fd, "QUIT\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);
}
