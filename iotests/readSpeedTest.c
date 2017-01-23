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
#include <sys/time.h>
#include <signal.h>

int keeprunning = 1;
int useDirect = 1;

typedef struct {
  int threadid;
  char *path;
  size_t total;
} threadInfoType;

struct timeval gettm()
{
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
}

double timedouble() {
  struct timeval now = gettm();
  double tm = (now.tv_sec * 1000000 + now.tv_usec);
  return tm/1000000.0;
}

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keeprunning = 0;
}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int fd = open(threadContext->path, O_RDONLY | (useDirect ? O_DIRECT : 0)); // may use O_DIRECT if required, although running for a decent amount of time is illuminating
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  fprintf(stderr,"opened %s\n", threadContext->path);
#define BUFSIZE (1024*1024)
  void *buf = NULL;
  if (posix_memalign(&buf, 4096, BUFSIZE)) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }	
  int rbytes = 0;
  size_t lastg = 0;
  double starttime = timedouble();
  while (((rbytes = read(fd, buf, BUFSIZE)) > 0) && keeprunning) {
    threadContext->total += rbytes;
    if (threadContext->total >> 30 != lastg) {
      lastg = threadContext->total >>30;
      fprintf(stderr,"read from '%s': %zd GB, speed %.1f MB/s\n", threadContext->path, lastg, (threadContext->total >> 20) / (timedouble() - starttime));
    }
  }
  if (rbytes < 0) {
    perror("weird problem");
  }
  fprintf(stderr,"finished. Total read from '%s': %zd bytes in %.1f seconds, %.2f MB/s\n", threadContext->path, threadContext->total, timedouble() - starttime, (threadContext->total >> 20) / (timedouble() - starttime));
  close(fd);
  free(buf);
  return NULL;
}

void startThreads(int argc, char *argv[]) {
  if (argc > 0) {
    size_t threads = argc - 1;
    pthread_t *pt = (pthread_t*) calloc((size_t) threads, sizeof(pthread_t));
    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}

    threadInfoType *threadContext = (threadInfoType*) calloc(threads, sizeof(threadInfoType));
    if (threadContext == NULL) {fprintf(stderr,"OOM(threadContext): \n");exit(-1);}
    double starttime = timedouble();
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	threadContext[i].threadid = i;
	threadContext[i].path = argv[i + 1];
	threadContext[i].total = 0;
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }
    size_t allbytes = 0;
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	pthread_join(pt[i], NULL);
	allbytes += threadContext[i].total;
      }
    }
    double elapsedtime = timedouble() - starttime;
    fprintf(stderr,"Total %zd bytes, time %lf seconds, read rate = %lf GB/sec\n", allbytes, elapsedtime, (allbytes/1024.0/1024/1024) / elapsedtime);
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
  fprintf(stderr,"direct=%d\n", useDirect);
  startThreads(argc, argv);
  return 0;
}
