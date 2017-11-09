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
#include <syslog.h>

#include "utils.h"
#include "aioOperations.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted
int    readySetGo = 0;
size_t blockSize = 1024*1024; // default to 1MiB
int    exitAfterSeconds = 10; // default timeout
size_t minMBPerSec = 10;
size_t maxMBPerSec = 3000;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  logSpeedType logSpeed;
} threadInfoType;


void intHandler(int d) {
  (void)d;
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}

static void *readThread(void *arg) {
  // ready
  while (!readySetGo) {
    usleep(1);
  }

  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  size_t sz = blockDeviceSize(threadContext->path);

  if (sz == 0) {
    //    fprintf(stderr,"empty\n");
    return NULL;
  }
  
  long ret = readNonBlocking(threadContext->path, blockSize, sz, exitAfterSeconds, 1);
  if (ret > 0) {
    threadContext->total = ret;
  } else {
    threadContext->total = 0;
  }

  return NULL;
}


static void *writeThread(void *arg) {
  // ready
  while (!readySetGo) {
    usleep(1);
  }

  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  size_t sz = blockDeviceSize(threadContext->path);
  if (sz == 0) {
    //    fprintf(stderr,"empty\n");
    return NULL;
  }

  long ret = writeNonBlocking(threadContext->path, blockSize, sz, exitAfterSeconds, 1);
  if (ret > 0) {
    threadContext->total = ret;
  } else {
    threadContext->total = 0;
  }
  
  return NULL;
}

void startThreads(int argc, char *argv[]) {
  if (argc > 0) {
    size_t threads = argc - 1;
    size_t *blockSz = calloc(threads, sizeof(size_t)); if(!blockSz) {perror("oom");exit(1);}
    double *readSpeeds = calloc(threads, sizeof(double)); if(!readSpeeds) {perror("oom");exit(1);}
    double *writeSpeeds = calloc(threads, sizeof(double)); if(!writeSpeeds) {perror("oom");exit(1);}
    size_t *readTotal = calloc(threads, sizeof(size_t)); if(!readTotal) {perror("oom");exit(1);}
    size_t *writeTotal = calloc(threads, sizeof(size_t)); if(!writeTotal) {perror("oom");exit(1);}
    pthread_t *pt = (pthread_t*) calloc((size_t) threads, sizeof(pthread_t));
    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}

    threadInfoType *threadContext = (threadInfoType*) calloc(threads, sizeof(threadInfoType));
    if (threadContext == NULL) {fprintf(stderr,"OOM(threadContext): \n");exit(-1);}

    // READ
    for (size_t i = 0; i < threads; i++) {
      char *path = argv[i + 1];
      //      const size_t len = strlen(path);
      if ((argv[i + 1][0] != '-') /*&& (!isdigit(argv[i + 1][len - 1]))*/ ) {
	blockSz[i] = blockDeviceSize(path);
      }
    }
    
    for (size_t i = 0; i < threads; i++) {
      char *path = argv[i + 1];
      //      const size_t len = strlen(path);
      threadContext[i].threadid = -1;
      if ((argv[i + 1][0] != '-') /*&& (!isdigit(argv[i + 1][len - 1]))*/ ) {
	threadContext[i].threadid = i;
	threadContext[i].path = path;
	logSpeedInit(&threadContext[i].logSpeed);
	threadContext[i].total = 0;
	pthread_create(&(pt[i]), NULL, readThread, &(threadContext[i]));
      }
    }
    size_t allbytes = 0;
    double allmb = 0;
    usleep(0.5*1000*1000);  // sleep then...
    readySetGo = 1; // go for it
    
    for (size_t i = 0; i < threads; i++) {
      if (threadContext[i].threadid >= 0) {
	pthread_join(pt[i], NULL);
	readTotal[i] = threadContext[i].total;
	volatile size_t t = readTotal[i];
	logSpeedAdd(&threadContext[i].logSpeed, t);
	const double s = logSpeedTime(&threadContext[i].logSpeed);

	const double speed = (t / s) / 1024.0 / 1024;
	if (t == 0 || s <= 0.1 || speed > 100000) { // 100GB/s sanity check
	  readSpeeds[i] = 0;
	} else {
	  readSpeeds[i] = (t / s) / 1024.0 / 1024;
	}
      }
      //      logSpeedFree(&threadContext[i].logSpeed);
    }

    readySetGo = 0; // don't start yet
    keepRunning = 1;

    // WRITE
    for (size_t i = 0; i < threads; i++) {
      if (threadContext[i].threadid >= 0) {
	logSpeedReset(&threadContext[i].logSpeed);
	threadContext[i].total = 0;
	pthread_create(&(pt[i]), NULL, writeThread, &(threadContext[i]));
      }
    }

    usleep(0.5*1000*1000);  // sleep then...
    readySetGo = 1; // go for it

    for (size_t i = 0; i < threads; i++) {
      if (threadContext[i].threadid >= 0) {
	pthread_join(pt[i], NULL);
	writeTotal[i] = threadContext[i].total;
	volatile size_t t = writeTotal[i];
	logSpeedAdd(&threadContext[i].logSpeed, t);
	allbytes += t;
	allmb += logSpeedMean(&(threadContext[i].logSpeed)) / 1024.0 / 1024;

	const double s = logSpeedTime(&threadContext[i].logSpeed);

	const double speed = (t / s) / 1024.0 / 1024;
	if (t == 0 || s <= 0.1 || speed > 100000) { // 100GB/s sanity check
	  writeSpeeds[i] = 0;
	} else {
	  writeSpeeds[i] = (t / s) / 1024.0 / 1024;
	}
	
      }
      logSpeedFree(&threadContext[i].logSpeed);
    }

    size_t numok = 0;
    FILE *fp = fopen("ok.txt", "wt");
    if (fp == NULL) {perror("ok.txt");exit(1);}

    // post thread join
    fprintf(stdout, "Path     \tGB\trMB\trMB/s\twMB\twMB/s\tQueue\n");
    for (size_t i = 0; i < threads; i++) {
      if (threadContext[i].threadid >= 0) {

	char abs[1000];
	ssize_t l = readlink(argv[i + 1], abs, 1000);
	if (l >= 1) {
	  abs[l] = 0;
	} else {
	  strcpy(abs, threadContext->path);
	}

	char *suffix = getSuffix(abs);
	char *qt = getScheduler(suffix);

	//	char *qt = queueType(abs);

	fprintf(stdout, "%s\t%.0lf\t%.0lf\t%.0lf\t%.0lf\t%.0lf\t%-10s", argv[i + 1], blockSz[i] / 1024.0 / 1024 / 1024, readTotal[i]/1024.0/1024,readSpeeds[i], writeTotal[i]/1024.0/1024,writeSpeeds[i], qt);
	fflush(stderr);
	free(qt);
	if ((readSpeeds[i] > minMBPerSec) && (readSpeeds[i] < maxMBPerSec) && (writeSpeeds[i] > minMBPerSec) && (writeSpeeds[i] < maxMBPerSec)) {
	  numok++;
	  fprintf(stdout, "\tOK\n");
	  fprintf(fp, "%s\n", argv[i + 1]);
	} else {
	  fprintf(stdout, "\tx\n");
	}
      }
    }
    fclose(fp);

    char *user = username();
    syslog(LOG_INFO, "%s - has %zd drives in ok.txt (stutools %s)", user, numok, VERSION);
    free(user);

    
    free(threadContext);
    free(pt);
    free(readSpeeds);
    free(writeSpeeds);
    free(readTotal);
    free(writeTotal);
  }
  //  shmemUnlink();
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "t:k:m:")) != -1) {
    switch (opt) {
    case 'k':
      blockSize = atoi(optarg) * 1024;
      if (blockSize < 1024) blockSize = 1024;
      break;
    case 't':
      exitAfterSeconds = atoi(optarg);
      break;
    case 'm':
      minMBPerSec = atoi(optarg);
      if (minMBPerSec > 100000) minMBPerSec = 0; // if too large then set to 0
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
  fprintf(stderr,"checking disks: blocksize=%zd (%zd KiB), timeout=%d, min=%zd MiB/s\n", blockSize, blockSize/1024, exitAfterSeconds, minMBPerSec);
  startThreads(argc, argv);
  return 0;
}
