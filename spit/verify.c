#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <fcntl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <signal.h>
#include <stdio.h>

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

  size_t threads = 256;
  size_t batches = 5000;

  if (argc <= 1) {
    fprintf(stdout,"*usage* ./spitchecker [ -D ] [ -t %zd ] [ -b %zd ] [ -n (don't sort)] positions* (or -)\n", threads, batches);
    exit(1);
  }


  int opt = 0;
  size_t o_direct = O_DIRECT;
  size_t sort = 1;
  size_t displayJSON = 0;
  int quiet = 0;
  int process = 1; // by default collapse and sort

  while ((opt = getopt(argc, argv, "Dt:njb:q")) != -1) {
    switch (opt) {
    case 'b':
      batches = atoi(optarg);
      if (batches < 1) {
	batches = 1;
      }
      break;
    case 'D':
      o_direct = 0;
      break;
    case 'j':
      displayJSON = 1;
      break;
    case 't':
      threads = atoi(optarg);
      break;
    case 'q':
      quiet++;
      break;
    case 'n':
      sort = 0;
      break;
    default:
      break;
    }
  }

  size_t numFiles = argc -optind;
  if (!quiet) fprintf(stderr,"*info* number of files %zd, threads set to %zd, sort %zd, batch size %zd\n", numFiles, threads, sort, batches);

  positionContainer *origpc = NULL;
  CALLOC(origpc, numFiles, sizeof(positionContainer));

  jobType job;

  for (int i= optind; i < argc; i++) {
    FILE *fp = NULL;
    if (strcmp(argv[i], "-") == 0) { 
      if (!quiet) fprintf(stderr,"*info* position file: (stdin)\n");
      process = 0; // turn off processing
      fp = stdin;
      setlinebuf(fp);
    } else {
      if (!quiet) fprintf(stderr,"*info* position file: %s\n", argv[i]);
      fp = fopen(argv[i], "rt");
      if (!fp) {perror(argv[i]); exit(1);}
    }
    if (fp) {
      if (fp == stdin) {
	size_t correct = 0, incorrect = 0, ioerrors = 0, errors = 0, jc = 0;
	size_t tot_cor = 0;
	do {
	  job = positionContainerLoadLines(&origpc[0], fp, batches);
	  if (job.count) {
	    errors += verifyPositions(&origpc[0], threads, &job, o_direct, 0, 0 /*runtime*/, &correct, &incorrect, &ioerrors, quiet, process);
	    tot_cor += correct;
	    fprintf(stderr,"*info* spitchecker totals: correct %zd, errors %zd\n", tot_cor, errors);
	  }
	  positionContainerFree(&origpc[0]);
	  jc = job.count;
	  jobFree(&job);
	} while (jc != 0);
	for (size_t i = 0; i < numFiles; i++) {
	  positionContainerFree(&origpc[i]);
	}
	free(origpc);
	origpc=NULL;
	if (errors) {
	  exit(1);
	}
	exit(0);
      } else {
	job = positionContainerLoad(&origpc[i - optind], fp);
      }
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
  int errors = verifyPositions(&pc, threads, &job, o_direct, 0, 0 /*runtime*/, &correct, &incorrect, &ioerrors, quiet, process);
  jobFree(&job);

  if (!keepRunning) {
    fprintf(stderr,"*warning* early verification termination\n");
  }

  positionContainerFree(&pc);
  for (size_t i = 0; i < numFiles; i++) {
    positionContainerFree(&origpc[i]);
  }
  free(origpc);
  origpc=NULL;

  if (displayJSON) {
    fprintf(stdout,"{\n\t\"correct\": \"%zd\",\n\t\"incorrect\": \"%zd\",\n\t\"ioerrors\": \"%zd\"\n}\n", correct, incorrect, ioerrors);
  }

  if (errors) {
    exit(1);
  } else {
    exit(0);
  }
}


