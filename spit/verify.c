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

  jobType job;
  
  for (size_t i = 0; i < numFiles; i++) {
    fprintf(stderr,"*info* position file: %s\n", argv[1 + i]);
    FILE *fp = fopen(argv[i+1], "rt");
    if (fp) {
      job = positionContainerLoad(&origpc[i], fp);
      positionContainerInfo(&origpc[i]);
      fclose(fp);
    }
  }

  if (origpc->sz == 0) {
    fprintf(stderr,"*warning* no positions to verify\n");
    exit(-1);
  }

  positionContainer pc = positionContainerMerge(origpc, numFiles);

  
  const size_t threads = 256;
  fprintf(stderr,"*info* starting verify, %zd threads\n", threads);

  // verify must be sorted
  int errors = verifyPositions(&pc, threads, &job);

  if (!keepRunning) {fprintf(stderr,"*warning* early verification termination\n");}

  for (size_t i = 0; i < numFiles; i++) {
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
  
  
