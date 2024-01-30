#ifndef __CLUSTER_H
#define __CLUSTER_H

#include <stddef.h>
#include <stdio.h>

typedef struct {
  char *name; // grabbed from onboard mac, immutable
  char *hostname; // can be dynamic
  char *ipaddress;
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
} clusterNodeType;


typedef struct {
  size_t id; // count
  size_t port;
  double latestchange;

  clusterNodeType **node;
} clusterType;


clusterType * clusterInit(const size_t port);
int clusterAddNode(clusterType *c, const char *nodename, const double createdtime);
int clusterAddNodesIP(clusterType *c, const char *nodename, const char *ip);
int clusterFindNode(clusterType *c, const char *nodename);
void clusterUpdateSeen(clusterType *c, size_t nodeid);

char *clusterDumpJSONString(clusterType *c);

void clusterDumpJSON(FILE *fp, clusterType *c);

void clusterSetNodeIP(clusterType *c, size_t nodeid, char *address);
char *clusterGetNodeIP(clusterType *c, size_t nodeid);
void clusterUpdateSeen(clusterType *c, const size_t nodeid);

#endif
