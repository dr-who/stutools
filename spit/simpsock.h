#ifndef __SIMPSOCK_H
#define __SIMPSOCK_H


int sockconnect(const char *ipaddress, const size_t port, const double time_out);
int socksetup(int fd, const int timeout_seconds);
int socksend(int fd, char *s, int flags, const int quiet);
int sockrec(int fd, char *buffer, int len, int flags, const int quiet);
int sockclose(int fd);

#endif
