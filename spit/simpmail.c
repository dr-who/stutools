// grabbed bits of this off Stack Overflow

#include <stdlib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <string.h>
#include <time.h>
#include <arpa/inet.h>
#include <assert.h>

unsigned long getDevRandomLong(void) {
    FILE *fp = fopen("/dev/random", "rb");
    unsigned long c = 0;
    if (fread(&c, sizeof(unsigned long), 1, fp) == 0) {
        //      perror("random");
        c = time(NULL);
        fprintf(stderr, "*info* random seed from clock\n");
    }
    fclose(fp);
    if (c == 0) {
        fprintf(stderr, "*warning* the random number was 0. That doesn't happen often.\n");
    }
    return c;
}

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



void thesend(int fd, char *s, int flags, const int quiet) {
  send(fd, s, strlen(s), flags);
  if (quiet == 0) fprintf(stderr,"C: %s", s);
}

void therec(int fd, char *buffer, int len, int flags, const int quiet) {
  ssize_t ret = recv(fd, buffer, len, flags);
  buffer[ret] = 0; // make string zero terminated
  if (quiet == 0) printf("S: %s", buffer);
}


void simpmailSend(int fd, const int quiet, char *from, char *fromname, char *to, char *cc, char *bcc, char *subject, char *htmlbody, char *plainbody) {
  if (fd < 0) {
    fprintf(stderr,"*error* no fd, not sending email\n");
    return;
  }

  char *buffer = calloc(25*1024*1024, 1); assert(buffer);

   int FLAGS = 0;

   // get response from Server after connection
   therec(fd, buffer, 1024, FLAGS, quiet);

   // get response from Server after HELO (program blocks here)
   thesend(fd, "HELO [127.0.0.1]\r\n", FLAGS, quiet);
   therec(fd, buffer, 1024, FLAGS, quiet);

   sprintf(buffer,"MAIL FROM: <%s>\r\n", from);
   thesend(fd, buffer, FLAGS, quiet);
   therec(fd, buffer, 1024, FLAGS, quiet);

   sprintf(buffer,"RCPT TO: <%s>\r\n", to);
   thesend(fd, buffer, FLAGS, quiet);
   therec(fd, buffer, 1024, FLAGS, quiet);

   if (cc) {
     sprintf(buffer,"RCPT TO: <%s>\r\n", cc);
     thesend(fd, buffer, FLAGS, quiet);
     therec(fd, buffer, 1024, FLAGS, quiet);
   }

   if (bcc) {
     sprintf(buffer,"RCPT TO: <%s>\r\n", bcc);
     thesend(fd, buffer, FLAGS, quiet);
     therec(fd, buffer, 1024, FLAGS, quiet);
   }

   thesend(fd, "DATA\r\n", FLAGS, quiet);
   therec(fd, buffer, 1024, FLAGS, quiet);

   long ll = time(NULL);
   char timestring[1000];
   struct tm tm = *localtime(&ll);
   strftime(timestring, 999, "%a, %d %b %Y %H:%M:%S %z (%Z)", &tm);

   sprintf(buffer,"Date: %s\r\n", timestring);
   thesend(fd, buffer, FLAGS, quiet);

   sprintf(buffer,"Return-Path: <%s>\r\n", from);
   thesend(fd, buffer, FLAGS, quiet);
   
   sprintf(buffer,"Message-ID: <%lu%s>\r\n", getDevRandomLong(), from);
   thesend(fd, buffer, FLAGS, quiet);
   
   if (fromname) {
     sprintf(buffer, "From: \"%s\" <%s>\r\n", fromname, from);
     thesend(fd, buffer, FLAGS, quiet);
   } else {
     sprintf(buffer, "From: <%s>\r\n", from);
     thesend(fd, buffer, FLAGS, quiet);
   }
     

   sprintf(buffer, "To: <%s>\r\n", to);
   thesend(fd, buffer, FLAGS, quiet);

   if (cc) {
     sprintf(buffer, "Cc: <%s>\r\n", cc);
     thesend(fd, buffer, FLAGS, quiet);
   }

   if (bcc) {
     sprintf(buffer, "Bcc: <%s>\r\n", bcc);
     thesend(fd, buffer, FLAGS, quiet);
   }

   sprintf(buffer, "Subject: %s\r\n", subject);
   thesend(fd, buffer, FLAGS, quiet);

   if (htmlbody && (plainbody == NULL)) {
     thesend(fd, "Content-Type: text/html; charset=UTF-8\r\n", FLAGS, quiet);
     
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, htmlbody, FLAGS, quiet);
   } else if (plainbody && (htmlbody == NULL)) {
     thesend(fd, "Content-Type: text/plain; charset=UTF-8\r\n", FLAGS, quiet);
     
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, plainbody, FLAGS, quiet);
   } else {
     thesend(fd, "MIME-Version: 1.0\r\n", FLAGS, quiet);
     thesend(fd, "Content-Type: multipart/alternative;boundary=boundary\r\n", FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, "This is a MIME encoded message.\r\n", FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, "--boundary\r\n", FLAGS, quiet);
     thesend(fd, "Content-Type: text/plain; charset=UTF-8\r\n", FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, plainbody, FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);

     thesend(fd, "--boundary\r\n", FLAGS, quiet);
     thesend(fd, "Content-Type: text/html; charset=UTF-8\r\n", FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, htmlbody, FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, "\r\n", FLAGS, quiet);
     thesend(fd, "--boundary\r\n", FLAGS, quiet);
   }
   
   thesend(fd, "\r\n", FLAGS, quiet);
   thesend(fd, ".\r\n", FLAGS, quiet);
   therec(fd, buffer, 1024, FLAGS, quiet);
   
   thesend(fd, "QUIT\r\n", FLAGS, quiet);
   therec(fd, buffer, 1024, FLAGS, quiet);

   free(buffer);
}
