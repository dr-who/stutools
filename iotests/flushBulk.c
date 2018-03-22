#define _DEFAULT_SOURCE
#define _GNU_SOURCE
 
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <assert.h>

#include <stdio.h>
#include <stdlib.h>
#include <signal.h>
#include <getopt.h>

#include <pthread.h>

#include "aioRequests.h"
#include "utils.h"

int keepRunning = 1;
int verbose = 0;
size_t maxThreads = 100;
size_t blockSize = 0;
size_t exitOnReady = 0;

typedef struct {
  size_t fd;
  size_t id;
  size_t written;
  size_t size;
  size_t max;
  char  *block;
} threadInfoType;

volatile size_t ready = 0;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;

}

static void *runThread(void *arg) {
  volatile threadInfoType *threadContext = (threadInfoType*)arg;
  

  if (threadContext->fd) {
    int w = write(threadContext->fd, threadContext->block, threadContext->size);
    if (w != threadContext->size) {perror("write");}
  }
  ready++;

  size_t count = 0;
  while (ready != threadContext->max && keepRunning && count < 10000) {
    count++;
    usleep(100);
  }
  if (exitOnReady && threadContext->id==0) {
    fprintf(stderr,"*exiting on ready*\n");
    exit(0);
  }
  
  if (threadContext->fd) {
    if (threadContext->id == 0) {
      fprintf(stderr,"fsync()... sent from %zd threads...", threadContext->max);
      fflush(stderr);
    }

    
    int f = fsync(threadContext->fd);
    if (f < 0) {perror("fsync");}
    int c = close(threadContext->fd);
    if (c < 0) {perror("close");}

    if (threadContext->id == 0) {
      fprintf(stderr,"\n");
    }
  }

  return NULL;
}


void startThreads(size_t *fdArray, size_t num, size_t numThreads, size_t blocksize) {
  
  pthread_t *pt;
  CALLOC(pt, numThreads, sizeof(pthread_t));
  assert(pt);

  threadInfoType *threadContext;
  CALLOC(threadContext, numThreads, sizeof(threadInfoType));
  assert(threadContext);

  for (size_t i = 0; i < numThreads; i++) {
    threadContext[i].fd = fdArray[i % num];
    threadContext[i].id = i;
    threadContext[i].max = num;
    threadContext[i].size = blocksize;
    threadContext[i].block = aligned_alloc(blocksize, blocksize); if (!threadContext[i].block) {fprintf(stderr,"oom!\n");exit(-1);}
    //    fprintf(stderr,"creating thread %zd\n", i);
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }

  for (size_t i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
  }

  free(threadContext);
  free(pt);
}


int handle_args(int argc, char *argv[]) {

  if (argc <= 1) {
    fprintf(stderr,"./flushBulk [-b byteBlockSize -T threads -V -E] ... devices\n");
    exit(-1);
  } else {
    int opt;
    while ((opt = getopt(argc, argv, "T:b:VE")) != -1) {
      switch (opt) {
      case 'V':
	verbose++;
	break;
      case 'E':
	exitOnReady = 1;
	break;
      case 'T':
	maxThreads = atoi(optarg);
	break;
      case 'b':
	blockSize = atoi(optarg);
	break;
      default:
	break;
      }
    }
    return optind;
  }
}


int main(int argc, char *argv[]) {
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);

  int opt = handle_args(argc, argv);
  if (argc - opt <= 0) {
    fprintf(stderr,"*error* no devices specified\n");
    exit(-1);
  }


  size_t *fdArray;
  CALLOC(fdArray, maxThreads, sizeof(size_t));
  //  [opt, argc)
  size_t fdCount = 0, count = 0;
  while (fdCount < maxThreads && count < 10000) {
    int fd = 0;
    int index = opt + (count%(argc-opt));
    //        fprintf(stderr,"Index %zd is %d %s\n", fdCount, index, argv[index]);
    fd = open(argv[index], O_RDWR | O_DIRECT | O_TRUNC);
    if (fd <= 0) {
      if (verbose) {
	perror(argv[index]); // keep going
      }
      //      fdArray[fdCount] = 0;
    } else {
      if (verbose) {
	fprintf(stderr,"[drive %zd] opened %s ok\n", fdCount+1, argv[index]);
      }
      fdArray[fdCount++] = fd;
    }
    count++;
  }
  if (fdCount != maxThreads) {
    fprintf(stderr,"*error* can't open enough files. %zd %zd\n", fdCount, maxThreads);
  } else {
    fprintf(stderr,"*info* flushBulk: %zd devices, blockSize %zd, maxThreads %zd, exitOnReady %zd\n", fdCount, blockSize, maxThreads, exitOnReady);
    startThreads(fdArray, fdCount, maxThreads, blockSize);
  }

  free(fdArray);

  return 0;
}

