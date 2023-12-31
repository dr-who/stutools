#ifndef __MSGS_H
#define __MSGS_H

#include "numList.h"

typedef struct {
    int id;
    int num;
    int serverport;
    //  double *gbps;
    double *lasttime;
    numListType *nl;
    char **ips;
    double starttime;
} threadMsgType;

void msgStartServer(const int serverport);
void msgClient(const char *ipaddress, size_t bufSizeToUse, const int serverport, const size_t threads);

#endif
