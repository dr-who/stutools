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

void usage(size_t threads, size_t batches, size_t exitearlyn) {
    fprintf(stdout,"Usage:\n  cat positions* | ./spitchecker [ options ] -\n");
    fprintf(stdout,"      or\n");
    fprintf(stdout,"          spitchecker [ options ] singlefile.pos\n");
    fprintf(stdout,"\nOptions:\n");
    fprintf(stdout,"   -4    Limit block verifications to first 4 KiB\n");
    fprintf(stdout,"   -D    turn off O_DIRECT\n");
    fprintf(stdout,"   -t n  Specify the number of verification threads to run in parallel (%zd)\n", threads);
    fprintf(stdout,"   -b n  Batch size for streaming IOs (%zd)\n", batches);
    fprintf(stdout,"   -n    Don't sort positions\n");
    fprintf(stdout,"   -E n  Exit after n errors in a thread (%zd). -E0 don't exit with errors.\n", exitearlyn);
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
  size_t batches = 1000000;
  size_t exitearlyn = 1;

  if (argc <= 1) {
    usage(threads, batches, exitearlyn);
    exit(-1);
  }


  int opt = 0;
  size_t o_direct = O_DIRECT;
  size_t sort = 1;
  size_t displayJSON = 0;
  int quiet = 0;
  int process = 1; // by default collapse and sort
  size_t overridesize = 0;

  while ((opt = getopt(argc, argv, "Dt:njb:q4E:h")) != -1) {
    switch (opt) {
    case 'h':
      usage(threads, batches, exitearlyn);
      exit(-1);
      break;
    case 'E':
      exitearlyn = atoi(optarg);
      break;
    case '4':
      overridesize = 4096;
      break;
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
  if (numFiles >=2) {
    usage(threads, batches, exitearlyn);   
    exit(1);
  }
  if (!quiet) {
    fprintf(stderr,"*info* spitchecker\n");
    fprintf(stderr,"*info* number of files %zd, threads set to %zd, sort %zd, batch size %zd, exitearly %zd\n", numFiles, threads, sort, batches, exitearlyn);
  }


  jobType job;

  size_t totallineerrors = 0;
  size_t correct = 0, incorrect = 0, ioerrors = 0, errors = 0, jc = 0;
  size_t tot_cor = 0;
  

  for (int i= optind; i < argc; i++) {
    FILE *fp = NULL;
    if (strcmp(argv[i], "-") == 0) { 
      process = 0; // turn off processing
      fp = stdin;

      struct stat bf;
      fstat(fileno(fp),  &bf);
      if (S_ISFIFO(bf.st_mode)) {
	fcntl(fileno(fp), F_SETPIPE_SZ, 0);
	int ld = fcntl(fileno(fp), F_GETPIPE_SZ);
	if (!quiet) fprintf(stderr,"*info* positions streamed from FIFO/pipe (buffer %d)\n", ld);
      } else {
	if (!quiet) fprintf(stderr,"*info* position file: (stdin)\n");
	setlinebuf(fp);
      }

    } else {
      if (!quiet) fprintf(stderr,"*info* position file: %s\n", argv[i]);
      fp = fopen(argv[i], "rt");
      if (!fp) {perror(argv[i]); exit(1);}
    }

    if (fp) {
      if (fp == stdin) {
	do {
	  positionContainer *origpc = NULL;
	  CALLOC(origpc, numFiles, sizeof(positionContainer)); assert(origpc);
	  size_t lineswitherrors = 0;
	  job = positionContainerLoadLines(&origpc[0], fp, batches, &lineswitherrors);
	  if (job.count) {
	    if (lineswitherrors) {
	      fprintf(stderr,"*error* %zd input errors\n", lineswitherrors);
	      totallineerrors += lineswitherrors;
	    }
	    errors += verifyPositions(&origpc[0], origpc[0].sz < threads ? origpc[0].sz : threads, &job, o_direct, 0, 0 /*runtime*/, &correct, &incorrect, &ioerrors, quiet, process, overridesize, exitearlyn);
	    tot_cor += correct;
	    fprintf(stderr,"*info* spitchecker totals: correct %zd, check errors %zd, io errors %zd, input line errors %zd\n", tot_cor, errors, ioerrors, totallineerrors);
	    if (exitearlyn && (errors > exitearlyn)) {
	      fprintf(stderr,"*error* too many errors\n");
	      break;
	    }
	    positionContainerFree(&origpc[0]);
	  }
	  jc = job.count;
	  jobFree(&job);
	  free(origpc);
	} while (jc != 0);
	fclose(fp);
	if (errors) {
	  exit(1);
	}
	exit(0);
      } else {
	  positionContainer *origpc = NULL;
	  CALLOC(origpc, numFiles, sizeof(positionContainer));
	  size_t lineswitherrors = 0;
	  job = positionContainerLoad(&origpc[i - optind], fp, &lineswitherrors);
	  
	  if (origpc->sz == 0) {
	    fprintf(stderr,"*error* no positions to verify\n");
	    exit(-1);
	  }
	  
	  positionContainer pc = positionContainerMerge(origpc, numFiles);
	  positionContainerCheckOverlap(&pc);
	  
	  
	  if (lineswitherrors) {
	    fprintf(stderr,"*error* note that the input contained %zd error lines\n", lineswitherrors);
	  }
	  fprintf(stderr,"*info* starting verify, %zd threads\n", threads);
	  
	  // verify must be sorted
	  size_t correct2, incorrect2, ioerrors2;
	  errors = verifyPositions(&pc, threads, &job, o_direct, 0, 0 /*runtime*/, &correct2, &incorrect2, &ioerrors2, quiet, process, overridesize, exitearlyn);
	  jobFree(&job);
	  
	  if (!keepRunning) {
	    fprintf(stderr,"*warning* early verification termination\n");
	  }
	  
	  positionContainerFree(&pc);
	  for (size_t ii = 0; ii < numFiles; ii++) {
	    positionContainerFree(&origpc[ii]);
	  }
	  free(origpc);
	  origpc=NULL;
	  
      }
      //      positionContainerInfo(&origpc[i]);
      fclose(fp);
    }
  }

  if (displayJSON) {
    fprintf(stdout,"{\n\t\"correct\": \"%zd\",\n\t\"incorrect\": \"%zd\",\n\t\"ioerrors\": \"%zd\"\n}\n", correct, incorrect, ioerrors);
  }

  if (errors) {
    exit(1);
  } else {
    exit(0);
  }
}


