#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <fcntl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <signal.h>

/**
 * verify.c
 *
 * the main() for ./verify, a standalone position verifier
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"
#include "blockVerify.h"
  
int verbose = 1;
int keepRunning = 1;
size_t waitEvery = 0;

// sorting function, used by qsort
static int seedcompare(const void *p1, const void *p2)
{
  const positionType *pos1 = (positionType*)p1;
  const positionType *pos2 = (positionType*)p2;
  if (pos1->seed < pos2->seed) return -1;
  else if (pos1->seed > pos2->seed) return 1;
  else {
    if (pos1->pos < pos2->pos) return -1;
    else if (pos1->pos > pos2->pos) return 1;
    else return 0;
  }
}


void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}

/**
 * main
 *
 */
int main(int argc, char *argv[]) {
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  if (argc <= 1) {
    fprintf(stderr,"*usage* ./verify spit*.txt\n");
    exit(1);
  }
  
  size_t numFiles = argc -1;

  positionContainer *origpc;
  CALLOC(origpc, numFiles, sizeof(positionContainer));

  for (size_t i = 0; i < numFiles; i++) {
    fprintf(stderr,"*info* position file: %s\n", argv[1 + i]);
    FILE *fp = fopen(argv[i+1], "rt");
    if (fp) {
      positionContainerLoad(&origpc[i], fp);
      positionContainerInfo(&origpc[i]);
      fclose(fp);
    }
  }

  if (origpc->sz == 0) {
    fprintf(stderr,"*warning* no positions to verify\n");
    exit(-1);
  }

  positionContainer pc = positionContainerMerge(origpc, numFiles);

  int fd = 0;
  if (pc.sz) {
    fd = open(pc.device, O_RDONLY | O_DIRECT);
    if (fd < 0) {perror(pc.device);exit(-2);}
  }


  const size_t threads = 64;
  fprintf(stderr,"*info* starting verify, %zd threads\n", threads);

  qsort(pc.positions, pc.sz, sizeof(positionType), seedcompare);
  // verify must be sorted
  int errors = verifyPositions(fd, &pc, threads);

  close(fd);

  if (!keepRunning) {fprintf(stderr,"*warning* early verification termination\n");}

  for (size_t i = 0; i < numFiles; i++) {
    free(origpc[i].device);
    free(origpc[i].string);
    positionContainerFree(&origpc[i]);
  }
  free(origpc); origpc=NULL;

  positionContainerFree(&pc);

  if (errors) {
    exit(1);
  } else {
    exit(0);
  }
}
  
  
