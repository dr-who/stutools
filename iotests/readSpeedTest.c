#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>

#include "utils.h"
#include "logSpeed.h"

int keeprunning = 1;
int useDirect = 1;
size_t BUFSIZE = 1024*1024;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  logSpeedType logSpeed;
} threadInfoType;


void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keeprunning = 0;
}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int fd = open(threadContext->path, O_RDONLY | O_DIRECT ); // may use O_DIRECT if required, although running for a decent amount of time is illuminating
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //fprintf(stderr,"opened %s\n", threadContext->path);


  int chunkSizes[1] = {BUFSIZE};
  int numChunks = 1;
  
  readChunks(fd, threadContext->path, chunkSizes, numChunks, 60, &threadContext->logSpeed, BUFSIZE, 1024*1024*1024);
  threadContext->total = threadContext->logSpeed.total;
		
  close(fd);

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
	logSpeedInit(&(threadContext[i].logSpeed));
	threadContext[i].total = 0;
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }
    size_t allbytes = 0;
    double allmb = 0;
    double maxtime = 0;
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	pthread_join(pt[i], NULL);
	allbytes += threadContext[i].total;
	allmb += logSpeedMean(&(threadContext[i].logSpeed)) / 1024.0 / 1024;
	if (logSpeedTime(&(threadContext[i].logSpeed)) > maxtime) {
	  maxtime = logSpeedTime(&(threadContext[i].logSpeed));
	}
      }
    }
    fprintf(stderr,"Total %zd bytes, time %.1lf seconds, sum mean = %.2lf MB/sec\n", allbytes, maxtime, allbytes/maxtime/1024.0/1024);
  }
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dD")) != -1) {
    switch (opt) {
    case 'd':
      fprintf(stderr,"USING DIRECT\n");
      useDirect = 1;
      break;
    case 'D':
      fprintf(stderr,"NOT USING DIRECT\n");
      useDirect = 0;
      break;
    }
  }
}

int main(int argc, char *argv[]) {
  handle_args(argc, argv);
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"direct=%d, blocksize=%zd\n", useDirect, BUFSIZE);
  startThreads(argc, argv);
  return 0;
}
