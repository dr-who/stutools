#include <stdio.h>
#include <malloc.h>

#include "cluster.h"

int keepRunning = 1;

int main() {
  char * s=clusterDumpJSON(NULL);
  printf("%s", s);
  free(s);

  clusterType *c = clusterInit(1600);

  s=clusterDumpJSON(c);
  printf("%s", s);
  free(s);
  
  clusterAddNode(c, "stu");
  clusterAddNode(c, "nic");
  clusterAddNode(c, "nic");

  clusterAddNodesIP(c, "stu", "172.17.0.2");
  clusterAddNodesIP(c, "stu", "172.17.3.2");
  s=clusterDumpJSON(c);
  printf("%s", s);
  free(s);

  return 0;
}
