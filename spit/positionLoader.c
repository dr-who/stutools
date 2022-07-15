#include <stdlib.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>


#include "positions.h"
#include "jobType.h"
#include "utils.h"
#include "aioRequests.h"

int verbose = 0;
int keepRunning = 1;

int main(int argc, char *argv[]) {

  jobType j;
  jobInit(&j);
  
  positionContainer pc;
  positionContainerInit(&pc, (size_t)(timedouble()*1000));

  for (int i = 1; i < argc; i++) {
    size_t added = positionContainerAddLinesFilename(&pc, &j, argv[i]);
    fprintf(stderr,"loaded %zd lines from %s\n", added, argv[i]);
  }
  positionContainerCollapse(&pc);

  for (size_t i = 0; i < pc.sz; i++) {
    if (pc.positions[i].action == 'W') {
      pc.positions[i].action = 'R';
      pc.positions[i].verify = 1;
    } else {
      pc.positions[i].action = 'S'; // skip
      pc.positions[i].verify = 0;
    }
  }

  positionContainerInfo(&pc);

  size_t ios, trb, twb, ior;
  int fd = open("/dev/sdi", O_RDWR);
  assert(fd > 0);
  aioMultiplePositions(&pc, pc.sz, timedouble() + 9e9, 0, 1, 256, 0, 0, 4096, &ios, &trb, &twb, 1, pc.sz, 0, fd, 0, &ior, 0, NULL, NULL, 1, 0, 0, 1);

  

  if (pc.sz > 0) {
    positionContainerFree(&pc);
    jobFree(&j);
  }
  
  return 0;
}
