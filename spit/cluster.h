#ifndef __CLUSTER_H
#define __CLUSTER_H

#include <stddef.h>
#include <stdio.h>
#include <semaphore.h>

#include "keyvalue.h"

typedef struct {
  char *name; // grabbed from onboard mac, immutable

  // the following should be KV
  char *nodename; // can be dynamic
  char *nodeOS; // can be dynamic
  char *ipaddress;
  char *biosdate;
  char *boardname;
  double created;
  double changed;
  double discovered;
  double seen;

  char *osrelease;
  size_t HDDcount;
  size_t HDDsizeGB;
  size_t SSDcount;
  size_t SSDsizeGB;
  size_t RAMGB;
  size_t Cores;

  
  keyvalueType *info;
} clusterNodeType;


typedef struct {
  sem_t sem;
  int localsmtp;
  char *alertToEmail;
  char *alertFromEmail;
  char *alertFromName;
  char *alertSubject;
  double alertLastTime;
  size_t alertCount;
  double downTime;
  
  int id; // count
  size_t port;
  double latestchange;

  clusterNodeType **node;
} clusterType;


clusterType * clusterInit(const size_t port);
int clusterAddNode(clusterType *c, const char *nodename, const double createdtime);
int clusterAddNodesIP(clusterType *c, const char *nodename, const char *ip);
int clusterFindNode(clusterType *c, const char *nodename);
void clusterUpdateSeen(clusterType *c, size_t nodeid);
void clusterSetAlertEmail(clusterType *c, char *toemail, char *fromemail, char *fromname, char *subject);
void clusterSendAlertEmail(clusterType *c);

char *clusterDumpJSONString(clusterType *c);
char * clusterGoodBad(clusterType *c, size_t *nodesGood, size_t *nodesBad);

void clusterDumpJSON(FILE *fp, clusterType *c);
double clusterCreated(clusterType *c);
void clusterSetNodeIP(clusterType *c, size_t nodeid, char *address);
char *clusterGetNodeIP(clusterType *c, size_t nodeid);
void clusterUpdateSeen(clusterType *c, const size_t nodeid);
void clusterChanged(clusterType *c, const size_t nodeid);

void clusterFree(clusterType **c);
#endif
