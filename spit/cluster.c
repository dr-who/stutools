
#include <malloc.h>
#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "cluster.h"
#include "utils.h"


// only change the times when an element changes value


// a cluster all has the same port
clusterType * clusterInit(const size_t port) {
  fprintf(stderr,"clusterInit on port %zd\n", port);
  clusterType *p = calloc(sizeof(clusterType), 1); assert(p);
  p->port = port;

  return p;
}

// returns -1 for can't find it
int clusterFindNode(clusterType *c, const char *nodename) {
  if (c) {
    for (int i = 0; i < c->id; i++) {
      if (c->node) {
	if (c->node[i]) {
	  if (strcasecmp(c->node[i]->name, nodename)==0) {
	    return i;
	  }
	}
      }
    }
  }
  return -1;
}



int clusterAddNode(clusterType *c, const char *nodename, const double createdtime) {
  const int index = c->id;
  for (int i = 0; i < index; i++) {
    if (strcasecmp(c->node[i]->name, nodename)==0) {
      fprintf(stderr,"*warning* node already there, ignoring\n");
      return i;
    }
  }

  c->id++;
  c->node = realloc(c->node, c->id * sizeof(clusterNodeType *));
  c->node[index] = calloc(sizeof(clusterNodeType), 1); // create cluster node
  memset(c->node[index], 0, sizeof(clusterNodeType));
  c->node[index]->name = strdup(nodename);
  c->node[index]->ipaddress = strdup(nodename);
  c->node[index]->created = createdtime;
  c->node[index]->discovered = timeAsDouble();
  c->node[index]->changed = c->node[index]->created;

  c->latestchange = c->node[index]->created;
  
  return index;
}

int clusterAddNodesIP(clusterType *c, const char *nodename, const char *ip) {
  fprintf(stderr,"addnodesip %s %s\n",nodename, ip);
  int find = clusterFindNode(c, nodename);
  if (find < 0) {
    printf("*error* can't find node. add it first\n");
    //    find = clusterAddNode(c, nodename);
    //    assert(find >= 0);
    return -1;
  }
  if ((c->node[find]->ipaddress==NULL) || (strcmp(c->node[find]->ipaddress, ip) != 0)) {
    //different
    free(c->node[find]->ipaddress);
    c->node[find]->ipaddress = strdup(ip);
    c->node[find]->changed = timeAsDouble();
    c->latestchange = c->node[find]->changed;
  }
  return find;
}

char *clusterDumpJSONString(clusterType *c) {
  char *buf = calloc(1,1000000);
  char *ret = buf;

  // sort
  if (c) 
    for (int i = 0; i < c->id-1; i++) {
      for (int j = i+1; j < c->id; j++) {
      if (strcmp(c->node[i]->ipaddress, c->node[j]->ipaddress) > 0) {
	// swap
	clusterNodeType t = *c->node[i];
	*c->node[i] = *c->node[j];
	*c->node[j] = t;
      }
    }
  }

  const double now = timeAsDouble();
  if (c) {
    buf += sprintf(buf, "{ \"clusterport\": %zd,\n", c->port);
    buf += sprintf(buf, "  \"latestchange\": %lf,\n", c->latestchange);
    buf += sprintf(buf, "  \"latestchangesecondsago\": %lf,\n", now - c->latestchange);
    buf += sprintf(buf, "  \"nodes\": [\n");
    for (int i = 0; i < c->id; i++) {
      buf += sprintf(buf, "    {\n");
      buf += sprintf(buf, "       \"node\": \"%s\",\n", c->node[i]->name);
      buf += sprintf(buf, "       \"nodename\": \"%s\",\n", c->node[i]->nodename);
      buf += sprintf(buf, "       \"nodeOS\": \"%s\",\n", c->node[i]->nodeOS);
      buf += sprintf(buf, "       \"boardname\": \"%s\",\n", c->node[i]->boardname);
      buf += sprintf(buf, "       \"biosdate\": \"%s\",\n", c->node[i]->biosdate);
      buf += sprintf(buf, "       \"lastseen\": %.0lf,\n", now - c->node[i]->seen);
      buf += sprintf(buf, "       \"created\": %lf,\n", c->node[i]->created);
      buf += sprintf(buf, "       \"age\": %lf,\n", now - c->node[i]->created);
      double delay = c->node[i]->discovered - c->node[i]->created;
      if (delay < 0) delay = 0;
      buf += sprintf(buf, "       \"discoveredafter\": %lf,\n", delay);
      buf += sprintf(buf, "       \"changed\": %lf,\n", c->node[i]->changed);
      buf += sprintf(buf, "       \"sincechanged\": %lf,\n", now - c->node[i]->changed);
      buf += sprintf(buf, "       \"ipaddress\": \"%s\",\n", c->node[i]->ipaddress ? c->node[i]->ipaddress : "n/a");
      buf += sprintf(buf, "       \"RAMGB\": %ld,\n", c->node[i]->RAMGB);
      buf += sprintf(buf, "       \"Cores\": %ld,\n", c->node[i]->Cores);
      buf += sprintf(buf, "       \"HDDcount\": %ld,\n", c->node[i]->HDDcount);
      buf += sprintf(buf, "       \"HDDsizeGB\": %ld,\n", c->node[i]->HDDsizeGB);
      buf += sprintf(buf, "       \"SSDcount\": %ld,\n", c->node[i]->SSDcount);
      buf += sprintf(buf, "       \"SSDsizeGB\": %ld\n", c->node[i]->SSDsizeGB);
      
      buf += sprintf(buf, "    }");
      if (i < c->id-1) buf += sprintf(buf, ",");
      buf += sprintf(buf, "\n");
    }
    buf += sprintf(buf, "  ]\n");
    buf += sprintf(buf, "}\n");
  }

  char *ret2 = strdup(ret);
  free(ret);
  return ret2;
}

      
void clusterDumpJSON(FILE *fp, clusterType *c) {
  char * s = clusterDumpJSONString(c);
  fprintf(fp, "%s", s);
  free(s);
}


void clusterSetNodeIP(clusterType *c, size_t nodeid, char *address) {
  if (c->node[nodeid]->ipaddress) free(c->node[nodeid]->ipaddress);
  
  c->node[nodeid]->ipaddress = strdup(address);
  c->node[nodeid]->changed = timeAsDouble();
  c->latestchange = timeAsDouble();
}

char *clusterGetNodeIP(clusterType *c, size_t nodeid) {
  return c->node[nodeid]->ipaddress;
}

void clusterUpdateSeen(clusterType *c, const size_t nodeid) {
  c->node[nodeid]->seen = timeAsDouble();
}

void clusterFree(clusterType *c) {
  const int index = c->id;
  for (int i = 0; i <index; i++) {
    free(c->node[i]->name);
    free(c->node[i]->nodename);
    free(c->node[i]->nodeOS);
    free(c->node[i]->boardname);
    free(c->node[i]->biosdate);
    free(c->node[i]->ipaddress);
    free(c->node[i]->osrelease);
    free(c->node[i]);
  }
  free(c->node);
  free(c);
}
   
