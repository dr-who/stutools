#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <sys/time.h>
#include <signal.h>
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
int keeprunning = 1;
void intHandler(int d) {
  fprintf(stderr,"got signal");
  keeprunning = 0;
}
static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int fd = open(threadContext->path, O_RDONLY);
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  fprintf(stderr,"opened %s\n", threadContext->path);
#define BUFSIZE (1024*1024)
  char *buf = calloc(1, BUFSIZE); if (!buf) {fprintf(stderr,"eek\n");exit(1);}
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
      threadContext[i].threadid = i;
      threadContext[i].path = argv[i + 1];
      threadContext[i].total = 0;
      pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
    }
    size_t allbytes = 0;
    for (size_t i = 0; i < threads; i++) {
      pthread_join(pt[i], NULL);
      allbytes += threadContext[i].total;
    }
    double elapsedtime = timedouble() - starttime;
    fprintf(stderr,"Total %zd bytes, time %lf seconds, read rate = %lf GB/sec\n", allbytes, elapsedtime, (allbytes/1024.0/1024/1024) / elapsedtime);
  }
}
int main(int argc, char *argv[]) {
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  startThreads(argc, argv);
  return 0;
}
