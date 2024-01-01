
#include <malloc.h>
#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "cluster.h"
#include "utils.h"


// only change the times when an element changes value


// a cluster all has the same port
clusterType * clusterInit(const size_t port) {
  clusterType *p = calloc(1, sizeof(clusterType)); assert(p);
  p->port = port;

  return p;
}

// returns -1 for can't find it
int clusterFindNode(clusterType *c, const char *nodename) {
  for (size_t i = 0; i < c->id; i++) {
    if (strcasecmp(c->node[i]->name, nodename)==0) {
      return i;
    }
  }
  return -1;
}


int clusterAddNode(clusterType *c, const char *nodename) {
  for (size_t i = 0; i < c->id; i++) {
    if (strcasecmp(c->node[i]->name, nodename)==0) {
      fprintf(stderr,"*warning* node already there, ignoring\n");
      return i;
    }
  }
  c->id++;
  c->node = realloc(c->node, c->id * sizeof(clusterNodeType *));
  c->node[c->id - 1] = calloc(1, sizeof(clusterNodeType)); // create cluster node
  c->node[c->id - 1]->name = strdup(nodename);
  c->node[c->id - 1]->created = timeAsDouble();
  c->node[c->id - 1]->updated = c->node[c->id - 1]->created;

  c->latestchange = c->node[c->id - 1]->created;
  
  return c->id - 1;
}

int clusterAddNodesIP(clusterType *c, const char *nodename, const char *ip) {
  int find = clusterFindNode(c, nodename);
  if (find < 0) {
    find = clusterAddNode(c, nodename);
    assert(find >= 0);
  }
  c->node[find]->ipaddress = strdup(ip);
  c->node[find]->updated = timeAsDouble();
  c->latestchange = c->node[find]->updated;
  return find;
}

char *clusterDumpJSONString(clusterType *c) {
  char *buf = calloc(1,1000000);
  char *ret = buf;
  
  buf += sprintf(buf, "{ \"clusterport\": %zd,\n", c->port);
  buf += sprintf(buf, "  \"latestchange\": %lf,\n", c->latestchange);
  buf += sprintf(buf, "  \"latestchangesecondsago\": %lf,\n", timeAsDouble() - c->latestchange);
  buf += sprintf(buf, "  \"nodes\": [\n");
  for (size_t i = 0; i < c->id; i++) {
    buf += sprintf(buf, "    {\n");
    buf += sprintf(buf, "       \"node\": \"%s\",\n", c->node[i]->name);
    buf += sprintf(buf, "       \"created\": %lf,\n", c->node[i]->created);
    buf += sprintf(buf, "       \"updated\": %lf,\n", c->node[i]->updated);
    buf += sprintf(buf, "       \"ipaddress\": \"%s\"\n", c->node[i]->ipaddress ? c->node[i]->ipaddress : "n/a");
    buf += sprintf(buf, "    }");
    if (i < c->id-1) buf += sprintf(buf, ",");
    buf += sprintf(buf, "\n");
  }
  buf += sprintf(buf, "  ]\n");
  buf += sprintf(buf, "}\n");

  char *ret2 = strdup(ret);
  free(ret);
  return ret2;
}

      
char * clusterDumpJSON(clusterType *c) {
  char * s = clusterDumpJSONString(c);
  return s;
}
