#ifndef __CLUSTER_H
#define __CLUSTER_H

#include <stddef.h>
#include <stdio.h>
#include <semaphore.h>

#include "keyvalue.h"

typedef struct {
  char *name; // grabbed from onboard mac, immutable

  double created;
  double changed;
  double discovered;
  double seen;

  double badsince;
  double nodedowntime;
  
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

  double downSince;
  double downTime;
  
  int id; // count
  size_t port;
  double latestchange;

  clusterNodeType **node;
} clusterType;


clusterType * clusterInit(const size_t port);
int clusterAddNode(clusterType *c, const char *node, const double createdtime);
int clusterFindNode(clusterType *c, const char *node);
void clusterUpdateSeen(clusterType *c, size_t nodeid);
void clusterSetAlertEmail(clusterType *c, char *toemail, char *fromemail, char *fromname, char *subject);
void clusterSendAlertEmail(clusterType *c);

char *clusterDumpJSONString(clusterType *c);
char * clusterGoodBad(clusterType *c, size_t *nodesGood, size_t *nodesBad);

void clusterDumpJSON(FILE *fp, clusterType *c);
double clusterCreated(clusterType *c);
void clusterUpdateSeen(clusterType *c, const size_t nodeid);
void clusterChanged(clusterType *c, const size_t nodeid);
size_t clusterNameToPort(const char *name);
size_t clusterDefaultPort(void);

  void clusterFree(clusterType **c);
#endif
