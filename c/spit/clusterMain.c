#include <stdio.h>
#include <malloc.h>
#include <string.h>

#include "cluster.h"
#include "utilstime.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  char * s=clusterDumpJSONString(NULL);
  printf("%s", s);
  free(s);

  
  clusterType *c = clusterInit(1600);
  printf("\n");
  s=clusterDumpJSONString(c);
  printf("%s", s);
  free(s);
  
  clusterAddNode(c, "stu", timeAsDouble());

  s=clusterDumpJSONString(c);
  printf("%s", s);
  free(s);

  clusterFree(&c);

  return 0;
}
