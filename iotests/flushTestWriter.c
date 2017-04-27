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

#define NEEDLE "!!!### 3.14159265 ###!!!"

int justOneWrite = 0;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  size_t startPosition;
  size_t exclusive;
  logSpeedType logSpeed;
} threadInfoType;

int keepRunning = 1;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int mode = O_WRONLY | O_TRUNC | O_DIRECT;
  int fd = open(threadContext->path, mode);
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }

  {
    char *s = aligned_alloc(4096, 65536);
    size_t x = 0;
    while (1) {
      x++;
      memset(s, ' ', 65536);
      double timecheck = timedouble();
      sprintf(s + (x % 4096), "%20f  %s%2d", timecheck, NEEDLE, threadContext->threadid);
      int w = lseek(fd, threadContext->startPosition, SEEK_SET);
      if (w < 0) {
	perror("seek");
	fprintf(stderr, "%d\n", w);
      }
      fprintf(stderr,"thread %d, pos=%10zd, writing %f\n", threadContext->threadid, threadContext->startPosition + (x % 4096), timecheck);
      w = write(fd, s, 4096);
      if (w < 0) {
	perror("write");
	fprintf(stderr, "%d\n", w);
      }
      if (justOneWrite) {
	fprintf(stderr,"write complete... sleeping 20 before flush()\n");
	sleep(20);
      }

      fsync(fd);
      usleep(10);
    }
    free(s);
  }
  
  return NULL;
}

void startThreads(int argc, char *argv[]) {
  if (argc > 0) {
    
    size_t threads = argc - 1;
    pthread_t *pt = (pthread_t*) calloc((size_t) threads, sizeof(pthread_t));
    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}

    threadInfoType *threadContext = (threadInfoType*) calloc(threads, sizeof(threadInfoType));
    if (threadContext == NULL) {fprintf(stderr,"OOM(threadContext): \n");exit(-1);}

    //    int startP = 0;
    
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	threadContext[i].path = argv[i + 1];
	threadContext[i].threadid = i;
	threadContext[i].exclusive = 0;
	    
	threadContext[i].startPosition = 65536 * i;

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
    free(threadContext);
    free(pt);
  }
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "1")) != -1) {
    switch (opt) {
    case '1':
      justOneWrite = 1;
      break;
    }
  }
}


int main(int argc, char *argv[]) {
  handle_args(argc, argv);
  startThreads(argc, argv);
  return 0;
}
