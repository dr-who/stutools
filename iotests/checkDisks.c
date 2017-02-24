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
#include <ctype.h>

#include "utils.h"
#include "aioOperations.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted
size_t blockSize = 1024*1024; // default to 1MiB
int    exitAfterSeconds = 5; // default timeout

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
  int fd = open(threadContext->path, O_RDONLY | O_EXCL | O_DIRECT); // may use O_DIRECT if required, although running for a decent amount of time is illuminating
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  size_t sz = blockDeviceSize(threadContext->path);
  //  fprintf(stderr,"opened %s, size %zd B (%.0lf GB)\n", threadContext->path, sz, sz / 1024.0 / 1024 / 1024);


  size_t ret = readNonBlocking(threadContext->path, blockSize, sz, exitAfterSeconds);
  //  fprintf(stderr,"bytes received %zd\n", ret);
  threadContext->total = ret;

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
      char *path = argv[i + 1];
      size_t len = strlen(path);
      threadContext[i].threadid = -1;
      if ((argv[i + 1][0] != '-') && (!isdigit(argv[i + 1][len - 1]))) {
	threadContext[i].threadid = i;
	threadContext[i].path = argv[i + 1];
	logSpeedInit(&threadContext[i].logSpeed);
	threadContext[i].total = 0;
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }
    size_t allbytes = 0;
    double allmb = 0;
    double maxtime = 0;
    for (size_t i = 0; i < threads; i++) {
      if (threadContext[i].threadid >= 0) {
	//      if (argv[i + 1][0] != '-') {
	pthread_join(pt[i], NULL);
	volatile size_t t = threadContext[i].total;
	logSpeedAdd(&threadContext[i].logSpeed, t);
	const double s = logSpeedTime(&threadContext[i].logSpeed);
	allbytes += t;
	allmb += logSpeedMean(&(threadContext[i].logSpeed)) / 1024.0 / 1024;
	if (s > maxtime) {
	  maxtime = s;
	}
	fprintf(stderr,"%s: %zd bytes in %.1lf is %.1lf B/s (%.1f MB/s)\n", argv[i + 1], t, s, t / s, (t / s) / 1024.0 / 1024);
      }
      logSpeedFree(&threadContext[i].logSpeed);
    }
    //    fprintf(stderr,"Total %zd bytes, time %.1lf seconds, sum of mean = %.1lf MiB/sec\n", allbytes, maxtime, allbytes/maxtime/1024.0/1024);
    free(threadContext);
    free(pt);
  }
  //  shmemUnlink();
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "t:k:")) != -1) {
    switch (opt) {
    case 'k':
      blockSize = atoi(optarg) * 1024;
      if (blockSize < 1024) blockSize = 1024;
      break;
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    }
  }
}

int main(int argc, char *argv[]) {
  handle_args(argc, argv);
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"checking disks, blocksize=%zd (%zd KiB), timeout=%d\n", blockSize, blockSize/1024, exitAfterSeconds);
  startThreads(argc, argv);
  return 0;
}
