#ifndef _SIMPMAIL_H
#define _SIMPMAIL_H


int simpmailConnect(const char *IPADDRESS);
void simpmailClose(int fd);

void simpmailSend(int fd, char *from, char *to, char *subject, char *body);


#endif

