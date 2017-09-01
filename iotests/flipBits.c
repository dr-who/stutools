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
#include <malloc.h>

#include "utils.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted

int    exitAfterSeconds = 60; // default timeout
double limitToGB = 0.01; // 10MiB
double sleepBetween = 1000000; // 1 second

typedef struct {
  int threadid;
  char *path;
  char *data;
  ssize_t datalen;
  size_t position;
  size_t checksum;
} threadInfoType;


void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  if (strcmp(threadContext->path, "/dev/sda") == 0) {
    fprintf(stderr,"*error* won't erase /dev/sda\n");
    exit(1);
  }
  
  int fd = open(threadContext->path, O_RDWR | O_DIRECT); // may use O_DIRECT if required, although running for a decent amount of time is illuminating
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //    fprintf(stderr,"opened %s\n", threadContext->path);

  // do stuff
    size_t maxread = 1024*1024;

  
  //  fprintf(stderr,"fd %d, max read %zd\n", fd, maxread);

    //    size_t randd = (((size_t)lrand48() % maxread) >> 10) << 10; // seek to 1K boundary

  char *buffer = aligned_alloc(65536, maxread); if (!buffer) {fprintf(stderr,"OOM\n");exit(1);}

  if (lseek(fd, threadContext->position, SEEK_SET) == -1) {
    perror("cannot seek");
  }

  //  fprintf(stderr,"%s to position %zd\n", threadContext->path, threadContext->position);

  ssize_t rb = read(fd, buffer, maxread);
  if (rb < 0) {
    perror("runThread");
  }

  size_t p = lrand48() % maxread;
  buffer[p] = 255 - buffer[p];

  fprintf(stdout,"%s, position %zd, rand %zd\n", threadContext->path, threadContext->position, p);

  if (lseek(fd, threadContext->position, SEEK_SET) == -1) {
    perror("cannot seek");
  }
  ssize_t wb = write(fd, buffer, maxread);
  if (wb < 0) {
    perror("write");
  }
  free(buffer);


  close(fd);
  return NULL;
}

void startThreads(int argc, char *argv[], int index) {
  if (argc - index > 0) {
    size_t threads = argc - index;

    pthread_t *pt;
    CALLOC(pt, threads, sizeof(pthread_t));

    threadInfoType *threadContext, *nextContext;
    CALLOC(threadContext, threads, sizeof(threadInfoType));
    CALLOC(nextContext, threads, sizeof(threadInfoType));


    
    double start = timedouble();

    while (((timedouble() - start) < exitAfterSeconds) && keepRunning) {

      // flip the bits
      for (size_t i = 0; i < threads; i++) {
	if (argv[i + index][0] != '-') {
	  threadContext[i].threadid = i;
	  threadContext[i].position = 0;

	  threadContext[i].path = argv[i + index];
	  pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
	}
      }

      for (size_t i = 0; i < threads; i++) {
	if (argv[i + index][0] != '-') {
	  pthread_join(pt[i], NULL);
	  fprintf(stdout,"path %s, position %zd, sz = %ld, checksum = %zd\n", threadContext[i].path, threadContext[i].position, threadContext[i].datalen, threadContext[i].checksum);
	}
      }

      usleep(sleepBetween);
      //      break;
    }
    //output info
    
    free(threadContext);
    free(nextContext);
    free(pt);
  }
}

int handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "G:t:s:")) != -1) {
    switch (opt) {
    case 'G':
      limitToGB = atof(optarg);
      break;
    case 's':
      sleepBetween = atof(optarg);
      break;
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    default:
      exit(-1);
    }
  }
  return optind;
}

int main(int argc, char *argv[]) {
  int index = handle_args(argc, argv);
  srand48((size_t) timedouble());
  
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"*info* flipBits: number of disks %d, sleepBetween=%.0lf ns, timeout=%d s, limit=%.2lf GiB\n", argc - index, sleepBetween,  exitAfterSeconds, limitToGB);
     
  startThreads(argc, argv, index);
  return 0;
}
