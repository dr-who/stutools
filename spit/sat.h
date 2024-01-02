#ifndef __SAT_H
#define __SAT_H

#include "numList.h"
#include "interfaces.h"

typedef struct {
  int id;
  int num;
  int serverport;
  char *eth;
  size_t speed;
  //  double *gbps;
  double *lasttime;
  numListType *nl;
  char *localhost;
  char *tryhost;
  double starttime;
  interfacesIntType *n;
  
} threadMsgType;

//void msgStartServer(const int serverport);
//void msgClient(const char *ipaddress, size_t bufSizeToUse, const int serverport, const size_t threads);

#endif
