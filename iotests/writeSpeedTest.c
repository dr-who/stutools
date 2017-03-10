#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>

#include "utils.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted
size_t blockSize = 1024*1024; // default to 1MiB
int    exitAfterSeconds = 60; // default timeout
int    useDirect = 1;
int    isSequential = 1;
int    verifyWrites = 0;
float  flushEveryGB = 0;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  logSpeedType logSpeed;
} threadInfoType;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}
static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int fd = open(threadContext->path, O_WRONLY | O_TRUNC | O_EXCL | (useDirect ? O_DIRECT : 0));
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //fprintf(stderr,"opened %s\n", threadContext->path);

  int chunkSizes[1] = {blockSize};
  int numChunks = 1;
  
  writeChunks(fd, threadContext->path, chunkSizes, numChunks, exitAfterSeconds, &threadContext->logSpeed, blockSize, OUTPUTINTERVAL, isSequential, useDirect, verifyWrites, flushEveryGB); // will close fd
  threadContext->total = threadContext->logSpeed.total;

  return NULL;
}

void startThreads(int argc, char *argv[]) {
  if (argc > 0) {
    
    size_t threads = argc - 1;
    pthread_t *pt = (pthread_t*) calloc((size_t) threads, sizeof(pthread_t));
    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}

    threadInfoType *threadContext = (threadInfoType*) calloc(threads, sizeof(threadInfoType));
    if (threadContext == NULL) {fprintf(stderr,"OOM(threadContext): \n");exit(-1);}
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	threadContext[i].threadid = i;
	threadContext[i].path = argv[i + 1];
	threadContext[i].total = 0;
	logSpeedInit(&threadContext[i].logSpeed);
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }

    
    size_t allbytes = 0;
    double maxtime = 0;
    double allmb = 0;
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	pthread_join(pt[i], NULL);
	allbytes += threadContext[i].total;
	allmb += logSpeedMean(&threadContext[i].logSpeed) / 1024.0 / 1024;
	if (logSpeedTime(&threadContext[i].logSpeed) > maxtime) {
	  maxtime = logSpeedTime(&threadContext[i].logSpeed);
	}
	logSpeedFree(&threadContext[i].logSpeed);
      }
    }
    fprintf(stderr,"Total %zd bytes, time %.1lf seconds, sum of mean = %.1lf MiB/sec\n", allbytes, maxtime, allmb);
    free(threadContext);
    free(pt);
  }
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDrt:k:vf:")) != -1) {
    switch (opt) {
    case 'k':
      blockSize=atoi(optarg) * 1024;
      if (blockSize < 1024) blockSize = 1024;
      break;
    case 'r': 
      isSequential = 0;
      break;
    case 'd':
      useDirect = 1;
      break;
    case 'D':
      useDirect = 0;
      break;
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    case 'f':
      flushEveryGB = atof(optarg);
      break;
    case 'v':
      verifyWrites = 1;
      break;
    default:
      exit(-1);
    }
    
  }
}


int main(int argc, char *argv[]) {
  handle_args(argc, argv);
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"direct=%d, blocksize=%zd (%zd KiB), %s, timeout=%d\n", useDirect, blockSize, blockSize/1024, isSequential ? "sequential" : "random", exitAfterSeconds);
  startThreads(argc, argv);
  return 0;
}
