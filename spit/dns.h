#ifndef __DNS_H
#define __DNS_H

#include "numList.h"

typedef struct {
  size_t num;
  char **dnsServer;
} dnsServersType;


void dnsServersInit(dnsServersType *d);

void dnsServersAdd(dnsServersType *d, const char *server);
void dnsServersAddFile(dnsServersType *d, const char *server, const char *regexstring);

void dnsServersRm(dnsServersType *d, size_t id);
void dnsServersClear(dnsServersType *d);
void dnsServersFree(dnsServersType *d);
void dnsServersDump(dnsServersType *d);

size_t dnsServersN(dnsServersType *d);



int dnsLookupA(char *hostname);

int dnsLookupAServer(char *hostname, char *dns_server);


  
#endif

