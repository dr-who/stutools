#include "diskStats.h"

#include <stdio.h>

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  
  diskStatType d;

  diskStatSetup(&d);
  diskStatLoadProc(&d);
  diskStatLoadProc(&d);
  diskStatFromFilelist(&d, "devices.txt", 0);
  diskStatInfo(&d);

  size_t sr = 0, sw = 0, tio = 0, ior = 0, iow = 0;
  diskStatUsage(&d, &sr, &sw, &tio, &ior, &iow);
  fprintf(stderr, "%zd %zd %zd %zd %zd %zd\n", d.deviceCount, sr, sw, tio, ior, iow);

  diskStatFree(&d);
  diskStatFree(&d);
  return 0;
}



