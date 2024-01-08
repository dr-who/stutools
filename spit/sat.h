#ifndef __SAT_H
#define __SAT_H

#include "interfaces.h"
#include "cluster.h"

typedef struct {
  int id;
  int num;
  int serverport;
  char *eth;
  char *fqn;
  double *lasttime;
  char *localhost;
  char *tryhost;
  double starttime;
  interfacesIntType *n;
  clusterType *cluster;
} threadMsgType;

//void msgStartServer(const int serverport);
//void msgClient(const char *ipaddress, size_t bufSizeToUse, const int serverport, const size_t threads);

#endif
