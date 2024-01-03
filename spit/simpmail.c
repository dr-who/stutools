// grabbed bits of this off Stack Overflow

#include <stdlib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <string.h>
#include <time.h>
#include <arpa/inet.h>
#include <assert.h>
#include <uuid/uuid.h>

#include "utils.h"

int sockconnect(const char *ipaddress, const size_t port, const double time_out){
   int sock = socket(AF_INET, SOCK_STREAM, 0);

   setsockopt(sock, SOL_SOCKET, SO_REUSEADDR | SO_REUSEPORT, NULL, 0);

   
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

// stub
int simpmailConnect(const char *ipaddress) { // SMTP is 25
  return sockconnect(ipaddress, 25, 5);
}


void simpmailClose(int fd) {
  shutdown(fd, 0);
  close(fd);
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


void simpmailSend(int fd, const int quiet, char *from, char *fromname, char *to, char *cc, char *bcc, char *subject, char *htmlbody, char *plainbody) {
  if (fd < 0) {
    fprintf(stderr,"*error* no fd, not sending email\n");
    return;
  }

  char *buffer = calloc(25*1024*1024, 1); assert(buffer);

   int FLAGS = 0;

   // get response from Server after connection
   sockrec(fd, buffer, 1024, FLAGS, quiet);

   // get response from Server after HELO (program blocks here)
   socksend(fd, "HELO [127.0.0.1]\r\n", FLAGS, quiet);
   sockrec(fd, buffer, 1024, FLAGS, quiet);

   sprintf(buffer,"MAIL FROM: <%s>\r\n", from);
   socksend(fd, buffer, FLAGS, quiet);
   sockrec(fd, buffer, 1024, FLAGS, quiet);

   sprintf(buffer,"RCPT TO: <%s>\r\n", to);
   socksend(fd, buffer, FLAGS, quiet);
   sockrec(fd, buffer, 1024, FLAGS, quiet);

   if (cc) {
     sprintf(buffer,"RCPT TO: <%s>\r\n", cc);
     socksend(fd, buffer, FLAGS, quiet);
     sockrec(fd, buffer, 1024, FLAGS, quiet);
   }

   if (bcc) {
     sprintf(buffer,"RCPT TO: <%s>\r\n", bcc);
     socksend(fd, buffer, FLAGS, quiet);
     sockrec(fd, buffer, 1024, FLAGS, quiet);
   }

   socksend(fd, "DATA\r\n", FLAGS, quiet);
   sockrec(fd, buffer, 1024, FLAGS, quiet);

   long ll = time(NULL);
   char timestring[1000];
   struct tm tm = *localtime(&ll);
   strftime(timestring, 999, "%a, %d %b %Y %H:%M:%S %z (%Z)", &tm);

   sprintf(buffer,"Date: %s\r\n", timestring);
   socksend(fd, buffer, FLAGS, quiet);

   sprintf(buffer,"Return-Path: <%s>\r\n", from);
   socksend(fd, buffer, FLAGS, quiet);


   uuid_t uuid;
   
   // generate
   uuid_generate_time_safe(uuid);
   
   // unparse (to string)
   char uuid_str[37];      // ex. "1b4e28ba-2fa1-11d2-883f-0016d3cca427" + "\0"
   uuid_unparse_lower(uuid, uuid_str);
   
   sprintf(buffer,"Message-ID: <%s%s>\r\n", uuid_str, from);
   socksend(fd, buffer, FLAGS, quiet);
   
   if (fromname) {
     sprintf(buffer, "From: \"%s\" <%s>\r\n", fromname, from);
     socksend(fd, buffer, FLAGS, quiet);
   } else {
     sprintf(buffer, "From: <%s>\r\n", from);
     socksend(fd, buffer, FLAGS, quiet);
   }
     

   sprintf(buffer, "To: <%s>\r\n", to);
   socksend(fd, buffer, FLAGS, quiet);

   if (cc) {
     sprintf(buffer, "Cc: <%s>\r\n", cc);
     socksend(fd, buffer, FLAGS, quiet);
   }

   if (bcc) {
     sprintf(buffer, "Bcc: <%s>\r\n", bcc);
     socksend(fd, buffer, FLAGS, quiet);
   }

   sprintf(buffer, "Subject: %s\r\n", subject);
   socksend(fd, buffer, FLAGS, quiet);

   if (htmlbody && (plainbody == NULL)) {
     socksend(fd, "Content-Type: text/html; charset=UTF-8\r\n", FLAGS, quiet);
     
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, htmlbody, FLAGS, quiet);
   } else if (plainbody && (htmlbody == NULL)) {
     socksend(fd, "Content-Type: text/plain; charset=UTF-8\r\n", FLAGS, quiet);
     
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, plainbody, FLAGS, quiet);
   } else {
     socksend(fd, "MIME-Version: 1.0\r\n", FLAGS, quiet);
     socksend(fd, "Content-Type: multipart/alternative;boundary=boundary\r\n", FLAGS, quiet);
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, "This is a MIME encoded message.\r\n", FLAGS, quiet);
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, "--boundary\r\n", FLAGS, quiet);
     socksend(fd, "Content-Type: text/plain; charset=UTF-8\r\n", FLAGS, quiet);
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, plainbody, FLAGS, quiet);
     socksend(fd, "\r\n", FLAGS, quiet);

     socksend(fd, "--boundary\r\n", FLAGS, quiet);
     socksend(fd, "Content-Type: text/html; charset=UTF-8\r\n", FLAGS, quiet);
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, htmlbody, FLAGS, quiet);
     socksend(fd, "\r\n", FLAGS, quiet);
     socksend(fd, "--boundary--\r\n", FLAGS, quiet);
   }
   
   socksend(fd, "\r\n", FLAGS, quiet);
   socksend(fd, ".\r\n", FLAGS, quiet);
   sockrec(fd, buffer, 1024, FLAGS, quiet);
   
   socksend(fd, "QUIT\r\n", FLAGS, quiet);
   sockrec(fd, buffer, 1024, FLAGS, quiet);

   free(buffer);
}
