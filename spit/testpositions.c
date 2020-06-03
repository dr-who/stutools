#define _POSIX_C_SOURCE 200809L

#include "positions.h"

#include <stdlib.h>
#include <malloc.h>

int verbose = 0;
int keepRunning = 0;

int main()
{
  positionContainer pc;
  positionContainerInit(&pc, 0);
  positionContainerSetup(&pc, 1);
  positionContainerInfo(&pc);
  positionContainerFree(&pc);

  positionContainerSetup(&pc, 1000);
  positionContainerInfo(&pc);
  positionContainerFree(&pc);

  size_t num = 10000;
  positionContainerSetup(&pc, num);
  positionContainerCreatePositions(&pc, 0, 1, 0, 0.0, 4096, 8192, 4096, 0, 0, 10000*10000, 42, 1, 0);
  positionContainerInfo(&pc);
  positionContainerDump(&pc, 10);

  positionContainerCheck(&pc, 0, 10000*10000, 0);

  //  positionContainerMultiply(&pc, 1);  positionContainerMultiply(&pc, 1);
  positionContainerInfo(&pc);

  positionContainer pc2 = positionContainerMultiply(&pc, 2);
  positionContainerInfo(&pc2);
  positionContainerFree(&pc);

  positionContainer pc3 = positionContainerMultiply(&pc2, 4);
  positionContainerInfo(&pc3);
  positionContainerFree(&pc2);


  positionContainerJumble(&pc3, 1);
  positionContainerInfo(&pc3);

  positionContainerCheck(&pc3, 0, pc3.maxbdSize, 0);
  positionContainerDump(&pc3, 10);

  for (size_t i = 0; i < pc3.sz; i++) {
    pc3.positions[i].finishTime = 1;
  }

  char *tmp = malloc(100);
  sprintf(tmp,"testfileXXXXXX");
  int ret = mkstemp(tmp);
  fprintf(stderr,"ret %d\n", ret);
  fprintf(stderr,"temp: %s\n", tmp);
  positionContainerSave(&pc3, tmp, 10000*10000, 0, NULL);
  positionContainerInfo(&pc3);
  //  free(tmp);

  positionContainerCollapse(&pc3);
  positionContainerInfo(&pc3);

  positionContainerDump(&pc3, 10);

  positionContainerFree(&pc3);
  positionContainerInfo(&pc3);

  deleteFile(tmp);
  free(tmp);


  return 0;
}


