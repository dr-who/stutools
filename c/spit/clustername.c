#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "cluster.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {

  size_t port = 0;
  if (argc < 2) {
    port = clusterDefaultPort();
  } else {
    fprintf(stderr,"*info* argv[1] = %s\n", argv[1]);
    port = clusterNameToPort(argv[1]);
  }
  printf("%zd\n", port);
  exit(0);
}
