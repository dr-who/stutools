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

int    keepRunning = 1;       // have we been interrupted
size_t blockSize = 1024*1024; // default to 1MiB
int    exitAfterSeconds = 60; // default timeout
int    resetSeconds = 20;
int    isSequential = 1;   
float  limitGBToProcess = 0;
int    offSetGB = 0;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  size_t startPosition;
  logSpeedType logSpeed;
} threadInfoType;


void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  int fd = open(threadContext->path, O_RDONLY | O_DIRECT); // may use O_DIRECT if required, although running for a decent amount of time is illuminating
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //fprintf(stderr,"opened %s\n", threadContext->path);

  lseek(fd, threadContext->startPosition, SEEK_SET);
  //  fprintf(stderr,"thread %d, lseek fd=%d to pos=%zd\n", threadContext->threadid, fd, threadContext->startPosition);

  int chunkSizes[1] = {blockSize};
  int numChunks = 1;
  
  readChunks(fd, threadContext->path, chunkSizes, numChunks, exitAfterSeconds, resetSeconds, &threadContext->logSpeed, blockSize, OUTPUTINTERVAL, isSequential, 1, limitGBToProcess); // will close fd
  threadContext->total = threadContext->logSpeed.total;

  return NULL;
}

void startThreads(int argc, char *argv[], int index) {
  if (argc - index > 0) {
    size_t threads = argc - index;
    pthread_t *pt = (pthread_t*) calloc((size_t) threads, sizeof(pthread_t));
    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}

    threadInfoType *threadContext = (threadInfoType*) calloc(threads, sizeof(threadInfoType));
    if (threadContext == NULL) {fprintf(stderr,"OOM(threadContext): \n");exit(-1);}

    int startP = 0;
    
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + index][0] != '-') {
	threadContext[i].threadid = i;

	threadContext[i].startPosition = (1024L*1024*1024) * startP;
	startP += offSetGB;

	threadContext[i].path = argv[i + index];
	logSpeedInit(&threadContext[i].logSpeed);
	threadContext[i].total = 0;
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }
    size_t allbytes = 0;
    double allmb = 0;
    double maxtime = 0;
    size_t minSpeed = (size_t)-1; // actually a big number
    size_t driveCount = 0;
    size_t zeroDrives = 0;
    char *minName = NULL;
    
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + index][0] != '-') {
	pthread_join(pt[i], NULL);
	allbytes += threadContext[i].total;
	const double speed = logSpeedMean(&threadContext[i].logSpeed);
	allmb += speed;
	if (threadContext[i].total > 0) {
	  if (speed < minSpeed) {minSpeed = speed; minName = threadContext[i].path;}
	  driveCount++;
	} else {
	  fprintf(stderr,"*warning* zero bytes to %s\n", threadContext[i].path);
	  zeroDrives++;
	}
	if (logSpeedTime(&(threadContext[i].logSpeed)) > maxtime) {
	  maxtime = logSpeedTime(&(threadContext[i].logSpeed));
	}
      }
      logSpeedFree(&threadContext[i].logSpeed);
    }
    fprintf(stdout, "*info* worst %s %.1lf MiB/s, synced case %.1lf MiB/s, %zd drives, %zd zero byte drives\n", minName ? minName : "", TOMiB(minSpeed), TOMiB(minSpeed * driveCount), driveCount, zeroDrives);
    fprintf(stdout, "Total %.1lf GiB bytes, time %.1lf s, sum of mean = %.1lf MiB/s\n", TOGiB(allbytes), maxtime, TOMiB(allmb));
    free(threadContext);
    free(pt);
  }
  //  shmemUnlink();
}

int handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "r:t:k:o:T:")) != -1) {
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
      if ((optarg[0] == '-') || (atof(optarg) < 0)) {
	fprintf(stderr,"missing argument for '%c' option\n", opt);
	exit(-2);
      }
      float l = atof(optarg); if (l < 0) l = 0;
      limitGBToProcess = l;
      break;
    case 'o':
      offSetGB = atoi(optarg);
      if (offSetGB < 0) {
	offSetGB = 0;
      }
      break;
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    case 'T':
      resetSeconds = atoi(optarg);
      break;
    default:
      exit(-1);
    }
  }
  return optind;
}

int main(int argc, char *argv[]) {
  int index = handle_args(argc, argv);
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"direct=%d, blocksize=%zd (%zd KiB), timeout=%d, resettime=%d, %s", 1, blockSize, blockSize/1024, exitAfterSeconds, resetSeconds, isSequential ? "sequential" : "random");
  if (!isSequential && limitGBToProcess > 0) {
    fprintf(stderr," (limit %.1lf GB)", limitGBToProcess);
  }
  fprintf(stderr,"\n");
     
  startThreads(argc, argv, index);
  return 0;
}
