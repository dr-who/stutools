#include <stdlib.h>

#include "positions.h"
#include "jobType.h"
#include "utils.h"

int verbose = 0;
int keepRunning = 1;

int main(int argc, char *argv[]) {

  jobType j;
  jobInit(&j);
  
  positionContainer pc;
  positionContainerInit(&pc, (size_t)(timedouble()*1000));

  positionContainerInfo(&pc);

  for (int i = 1; i < argc; i++) {
    size_t added = positionContainerAddLinesFilename(&pc, &j, argv[i]);
    fprintf(stderr,"loaded %zd lines from %s\n", added, argv[i]);
  }
  positionContainerInfo(&pc);

  positionContainerDump(&pc, 20);

  positionContainerCollapse(&pc);
  positionContainerDump(&pc, 20);

  positionContainerInfo(&pc);

  if (pc.sz > 0) {
    positionContainerFree(&pc);
    jobFree(&j);
  }
  
  return 0;
}
