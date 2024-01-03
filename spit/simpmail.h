#ifndef _SIMPMAIL_H
#define _SIMPMAIL_H


int simpmailConnect(const char *IPADDRESS);
void simpmailClose(int fd);

void simpmailSend(int fd, const int quiet, char *fromemail, char *fromname, char *to, char *cc, char *bcc, char *subject, char *htmlbody, char *plainbody);

int sockconnect(const char *ipaddress, const size_t port);
int socksetup(int fd, const int timeout_seconds);
int socksend(int fd, char *s, int flags, const int quiet);
int sockrec(int fd, char *buffer, int len, int flags, const int quiet);


#endif

