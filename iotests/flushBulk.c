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
size_t maxThreads = 1000;
size_t blockSize = 0;

typedef struct {
  size_t fd;
  size_t id;
  size_t written;
  size_t size;
  size_t max;
  char  *block;
} threadInfoType;

volatile int ready = 0;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;

}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  
  if (threadContext->fd) {
    int w = write(threadContext->fd, threadContext->block, threadContext->size);
    if (w != threadContext->size) {perror("write");}
  }
  
  ready++;
  while (ready != threadContext->max && keepRunning) {
    usleep(1);
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
    fprintf(stderr,"./flushBulk [-b byteBlockSize -T threads] ... devices\n");
    exit(-1);
  } else {
    int opt;
    while ((opt = getopt(argc, argv, "T:b:")) != -1) {
      switch (opt) {
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

  fprintf(stderr,"*info* flushBulk: %d devices, blockSize %zd, maxThreads %zd\n", argc-opt, blockSize, maxThreads);

  size_t *fdArray;
  CALLOC(fdArray, maxThreads, sizeof(size_t));
  //  [opt, argc)
  int fdCount = 0;
  for (size_t i = 0; i < maxThreads; i++) {
    int fd = 0;
    int index = opt + (i%(argc-opt));
    //    fprintf(stderr,"Index %zd is %d\n", i, index);
    fd = open(argv[index], O_RDWR | O_DIRECT | O_TRUNC);
    if (fd <= 0) {
      perror(argv[index]); // keep going
      fdArray[fdCount] = 0;
    } else {
      //      fprintf(stderr,"opened %s ok\n", argv[index]);
      fdArray[fdCount] = fd;
    }
    fdCount++;
  }

  startThreads(fdArray, fdCount, maxThreads, blockSize);

  free(fdArray);

  return 0;
}

