#include <stdio.h>
#include <malloc.h>

#include "cluster.h"

int keepRunning = 1;

int main() {
  clusterType *c = clusterInit(1600);
  clusterAddNode(c, "stu");
  clusterAddNode(c, "nic");
  clusterAddNode(c, "nic");

  clusterAddNodesIP(c, "stu", "172.17.0.2");
  clusterAddNodesIP(c, "stu", "172.17.3.2");
  char * s=clusterDumpJSON(c);
  printf("%s", s);
  free(s);

  return 0;
}
