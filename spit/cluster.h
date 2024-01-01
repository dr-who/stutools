#ifndef __CLUSTER_H
#define __CLUSTER_H

#include <stddef.h>

typedef struct {
  char *name; // uuid
  char *ipaddress;
  double created;
  double updated;

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
char *clusterDumpJSON(clusterType *c);


#endif
