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


void intHandler(int d)
{
  if (d) {}
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}

/**
 * main
 *
 */
int main(int argc, char *argv[])
{
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  if (argc <= 1) {
    fprintf(stdout,"*usage* ./spitchecker [ -D ] [ -t 256 ] [ -n (don't sort)] positions*\n");
    exit(1);
  }

  size_t threads = 256;

  int opt = 0;
  size_t o_direct = O_DIRECT;
  size_t sort = 1;
  size_t displayJSON = 0;

  while ((opt = getopt(argc, argv, "Dt:nj")) != -1) {
    switch (opt) {
    case 'D':
      o_direct = 0;
      break;
    case 'j':
      displayJSON = 1;
      break;
    case 't':
      threads = atoi(optarg);
      break;
    case 'n':
      sort = 0;
      break;
    default:
      break;
    }
  }

  size_t numFiles = argc -optind;
  fprintf(stderr,"*info* number of files %zd, threads set to %zd, sort %zd\n", numFiles, threads, sort);

  positionContainer *origpc;
  CALLOC(origpc, numFiles, sizeof(positionContainer));

  jobType job;

  for (int i= optind; i < argc; i++) {
    fprintf(stderr,"*info* position file: %s\n", argv[i]);
    FILE *fp = fopen(argv[i], "rt");
    if (fp) {
      job = positionContainerLoad(&origpc[i - optind], fp);
      //      positionContainerInfo(&origpc[i]);
      fclose(fp);
    }
  }

  if (origpc->sz == 0) {
    fprintf(stderr,"*warning* no positions to verify\n");
    exit(-1);
  }

  positionContainer pc = positionContainerMerge(origpc, numFiles);
  positionContainerCheckOverlap(&pc);


  fprintf(stderr,"*info* starting verify, %zd threads\n", threads);

  // verify must be sorted
  size_t correct, incorrect, ioerrors;
  int errors = verifyPositions(&pc, threads, &job, o_direct, 0, 0 /*runtime*/, &correct, &incorrect, &ioerrors);

  if (!keepRunning) {
    fprintf(stderr,"*warning* early verification termination\n");
  }

  for (size_t i = 0; i < numFiles; i++) {
    positionContainerFree(&origpc[i]);
  }
  free(origpc);
  origpc=NULL;

  positionContainerFree(&pc);

  if (displayJSON) {
    fprintf(stdout,"{\n\t\"correct\": \"%zd\",\n\t\"incorrect\": \"%zd\",\n\t\"ioerrors\": \"%zd\"\n}\n", correct, incorrect, ioerrors);
  }

  if (errors) {
    exit(1);
  } else {
    exit(0);
  }
}


