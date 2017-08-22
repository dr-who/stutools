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
float  limitGBToProcess = 0;
int    offSetGB = 0;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  size_t startPosition;
  size_t exclusive;
  logSpeedType logSpeed;
} threadInfoType;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}
static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int mode = O_WRONLY | O_TRUNC | (useDirect ? O_DIRECT : 0);
  if (threadContext->exclusive) {
    mode = mode | O_EXCL;
  }
  int fd = open(threadContext->path, mode);
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //fprintf(stderr,"opened %s\n", threadContext->path);

  lseek(fd, threadContext->startPosition, SEEK_SET);
  //fprintf(stderr,"thread %d, lseek fd=%d to pos=%zd\n", threadContext->threadid, fd, threadContext->startPosition);

  int chunkSizes[1] = {blockSize};
  int numChunks = 1;
  
  writeChunks(fd, threadContext->path, chunkSizes, numChunks, exitAfterSeconds, &threadContext->logSpeed, blockSize, OUTPUTINTERVAL, isSequential, useDirect, limitGBToProcess, verifyWrites, flushEveryGB); // will close fd
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

    int startP = 0;
    
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	threadContext[i].path = argv[i + 1];
	threadContext[i].threadid = i;
	threadContext[i].exclusive = 1;
	// check previous to see if they are the same, if so make it non-exclusive
	for (size_t j = 0; j <= i; j++) {
	  if (strcmp(threadContext[i].path, argv[j]) == 0) {
	    threadContext[i].exclusive = 0;
	  }
	}

	//	fprintf(stderr,"path %s excl %zd\n", threadContext[i].path, threadContext[i].exclusive);
	    
	threadContext[i].startPosition = (1024L*1024*1024) * startP;
	startP += offSetGB;

	threadContext[i].total = 0;
	logSpeedInit(&threadContext[i].logSpeed);
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }

    
    size_t allbytes = 0;
    double maxtime = 0;
    double allmb = 0;
    size_t minSpeed = (size_t)-1; // actually a big number
    size_t driveCount = 0;
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + 1][0] != '-') {
	pthread_join(pt[i], NULL);
	driveCount++;
	const double speed = logSpeedMean(&threadContext[i].logSpeed);
	allbytes += threadContext[i].total;
	allmb += speed;
	if (speed < minSpeed) minSpeed = speed;
	if (logSpeedTime(&threadContext[i].logSpeed) > maxtime) {
	  maxtime = logSpeedTime(&threadContext[i].logSpeed);
	}
	logSpeedFree(&threadContext[i].logSpeed);
      }
    }
    fprintf(stderr,"Total %.1lf GiB bytes, time %.1lf s, sum of mean = %.1lf MiB/s (worse case %.1lf MiB/s, %zd drives)\n", TOGiB(allbytes), maxtime, TOMiB(allmb), TOMiB(minSpeed * driveCount), driveCount);
    free(threadContext);
    free(pt);
  }
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDr:t:k:vf:o:")) != -1) {
    switch (opt) {
    case 'k':
      if (optarg[0] == '-') {
	fprintf(stderr,"missing argument for '%c' option\n", opt);
	exit(-2);
      }
      if (atoi(optarg) <= 0) {
	fprintf(stderr,"incorrect argument for '%c' option\n", opt);
	exit(-2);
      }
      blockSize=atoi(optarg) * 1024;
      break;
    case 'r': 
      isSequential = 0;
      if (optarg[0] == '-') {
	fprintf(stderr,"missing argument for '%c' option\n", opt);
	exit(-2);
      }
      float l = atof(optarg); if (l < 0) l = 0;
      limitGBToProcess = l;
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
    case 'o':
      offSetGB = atoi(optarg);
      if (offSetGB < 0) {
	offSetGB = 0;
      }
      break;
    case 'v':
      verifyWrites = 1;
      break;
    case ':':
      fprintf(stderr,"argument missing for %c \n", optopt);
      exit(-2);
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
  fprintf(stderr,"direct=%d, blocksize=%zd (%zd KiB), timeout=%d, %s", useDirect, blockSize, blockSize/1024, exitAfterSeconds, isSequential ? "sequential" : "random");
  if (!isSequential && limitGBToProcess > 0) {
    fprintf(stderr," (limit %.1lf GB)", limitGBToProcess);
  }
  fprintf(stderr,"\n");

  startThreads(argc, argv);
  return 0;
}
