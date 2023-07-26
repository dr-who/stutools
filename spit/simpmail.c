// grabbed bits of this off Stack Overflow

#include <stdlib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <string.h>
#include <arpa/inet.h>

int initConnect(const char *IPADDRESS){
   int sock = socket(AF_INET, SOCK_STREAM, 0);

   setsockopt(sock, SOL_SOCKET, SO_REUSEADDR | SO_REUSEPORT, NULL, 0);

   struct sockaddr_in serv_addr;
   memset(&serv_addr, 0, sizeof(serv_addr));
   serv_addr.sin_port = htons(25);
   serv_addr.sin_family = AF_INET;

   if(inet_pton(AF_INET, IPADDRESS, &serv_addr.sin_addr) > 0){
      connect(sock, (struct sockaddr*)&serv_addr, sizeof(serv_addr));
   } else{
      return -1;
   }
   return sock;
 }


void thesend(int fd, char *s, int flags) {
  send(fd, s, strlen(s), flags);
  fprintf(stderr,"C: %s\n", s);
}

void therec(int fd, char *buffer, int len, int flags) {
  memset(buffer, 0, 1024);
  recv(fd, buffer, len, flags);
  printf("S: %s", buffer);
}


int main(void){

   char buffer[1024];

   memset(buffer, 0, sizeof(buffer));
   int FLAGS = 0;

   int fd;
   fd = initConnect("127.0.0.1");

   if(fd == -1){
      printf("Error occured while connecting to the Server");
      exit(1);
   }

   // get response from Server after connection
   therec(fd, buffer, 1024, FLAGS);

   // get response from Server after HELO (program blocks here)
   thesend(fd, "HELO 127.0.0.1\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);
   
   thesend(fd, "MAIL FROM: <no-reply@studeb.interspeed.co.nz>\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   thesend(fd, "RCPT TO: <stuart+test@netvalue.nz>\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   thesend(fd, "DATA\r\n", FLAGS);
   therec(fd, buffer, 1024, FLAGS);

   thesend(fd, "Date: Tue, 25 July 2023 12:01:00\r\n", FLAGS);
   thesend(fd, "From: \"Stuart Inglis, Ph.D.\" <stuart@netvalue.nz>, \"Nic Inglis\" <nic@ingcap.co.nz>\r\n", FLAGS);
   thesend(fd, "To: Stuart Inglis <stuart@netvalue.nz>\r\n", FLAGS);
   thesend(fd, "Subject: update\r\n", FLAGS);
   thesend(fd, "Content-Type: text/html\r\n", FLAGS);
   
   thesend(fd, "\r\n", FLAGS);
   thesend(fd, "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"https://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\">\r\n", FLAGS);
   thesend(fd, "<html xmlns=\"https://www.w3.org/1999/xhtml\">\r\n", FLAGS);
  thesend(fd, "<head><style type=\"text/css\">body{width:100% !important; -webkit-text-size-adjust:100%; -ms-text-size-adjust:100%; margin:0; padding:0;}  .ExternalClass {width:100%;} .ExternalClass, .ExternalClass p, .ExternalClass span, .ExternalClass font, .ExternalClass td, .ExternalClass div {line-height: 100%;} </style>\r\n", FLAGS);
   thesend(fd, "<body style=\"background-color:white;\"><h1>This is a test2</h1><p> www.netvalue.nz\r\n", FLAGS);
   thesend(fd, "This is a test2. <a href=\"http://www.netvalue.nz/\"Net V</a>\r\n", FLAGS);
   thesend(fd, "</body></html>\r\n", FLAGS);
   thesend(fd, ".\r\n", FLAGS);
   thesend(fd, "QUIT", FLAGS);

   therec(fd, buffer, 1024, FLAGS);
   //   recBytes = recv(fd, buffer, 1024, FLAGS);

   shutdown(fd, 0);

   exit(0);
}
