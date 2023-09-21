#ifndef _SIMPMAIL_H
#define _SIMPMAIL_H


int simpmailConnect(const char *IPADDRESS);
void simpmailClose(int fd);

void simpmailSend(int fd, char *fromemail, char *fromname, char *to, char *cc, char *bcc, char *subject, char *body);


#endif

