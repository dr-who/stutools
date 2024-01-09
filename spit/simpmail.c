// grabbed bits of this off Stack Overflow

#include <stdlib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <string.h>
#include <time.h>
#include <arpa/inet.h>
#include <assert.h>
#include <uuid/uuid.h>
#include <unistd.h>

#include "simpsock.h"

int simpmailConnect(const char *ipaddress) { // SMTP is 25
  return sockconnect(ipaddress, 25, 5);
}


void simpmailClose(int fd) {
  shutdown(fd, 0);
  close(fd);
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
