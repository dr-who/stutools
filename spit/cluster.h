#ifndef __CLUSTER_H
#define __CLUSTER_H

#include <stddef.h>
#include <stdio.h>

typedef struct {
  char *name; // uuid
  char *ipaddress;
  double created;
  double updated;
  double age;
  double expires;

  char *osrelease;
  size_t cores;
  size_t ram;
  size_t hdds;
  size_t hddsTB;
  size_t ssds;
  size_t ssdsTB;
} clusterNodeType;


typedef struct {
  size_t id; // count
  size_t port;
  double latestchange;

  clusterNodeType **node;
} clusterType;


clusterType * clusterInit(const size_t port);
int clusterAddNode(clusterType *c, const char *nodename);
int clusterAddNodesIP(clusterType *c, const char *nodename, const char *ip);
int clusterFindNode(clusterType *c, const char *nodename);

char *clusterDumpJSONString(clusterType *c);

void clusterDumpJSON(FILE *fp, clusterType *c);

void clusterSetNodeAge(clusterType *c, size_t nodeid, double age);
void clusterSetNodeExpires(clusterType *c, size_t nodeid, double expires);
void clusterSetNodeIP(clusterType *c, size_t nodeid, char *address);

#endif
