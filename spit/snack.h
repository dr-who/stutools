#ifndef _SNACK_H
#define _SNACK_H

#include <string.h>
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
} threadInfoType;

typedef struct {
    char *path;
    int speed;
    long lastrx, lasttx;
    double lasttime;
    long thisrx, thistx;
    double thistime;
    int mtu, numa;
    long thisrxerrors, thistxerrors, carrier_changes;
} stringType;

void startSnack(size_t num);

void dumpEthernet(void);

#endif
