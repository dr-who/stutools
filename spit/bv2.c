#define _XOPEN_SOURCE 500

#define _POSIX_C_SOURCE 200809L
#define _DEFAULT_SOURCE
#define _GNU_SOURCE

/**
 * blockVerify2.c
 *
 *
 */
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <assert.h>

#include "utils.h"
#include "positions.h"




typedef struct {
  size_t batchsize;
  size_t type; // 0 as is, 1 sorted, 2 random
  size_t o_direct;
  size_t threads;
  size_t isSetup;
  size_t inQueue;

  size_t numPositions;
  size_t maxAllocPositions;
  size_t nextpos;
  
  size_t inputComplete;
  
  positionType *p;
} blockVerifyType;


typedef struct {
  size_t id;
  size_t pleaseStop;
  blockVerifyType *b;
} threadInfoType;

pthread_t *pt;
threadInfoType *threadContext;
pthread_mutex_t mutex;




inline double timedouble()
{
  struct timeval now;
  gettimeofday(&now, NULL);
  double tm = ((double)now.tv_sec * 1000000.0) + now.tv_usec;
  assert(tm > 0);
  return tm/1000000.0;
}

void addPosition(blockVerifyType *b, size_t pos) {
  assert(b);
  pthread_mutex_lock(&mutex);
  //  fprintf(stderr,"*added* \n");
  b->numPositions = b->numPositions+1;
  pthread_mutex_unlock(&mutex);
}
  
  


size_t isNextPosition(blockVerifyType *b, size_t *next) {
  size_t isnext = 0;
  pthread_mutex_lock(&mutex);
  if (b->nextpos < b->numPositions) {
    isnext = 1;
    *next = b->nextpos;
    //    fprintf(stderr, "---%zd\n", nextpos);
    b->nextpos++;
  }
  pthread_mutex_unlock(&mutex);
  return isnext;
}

static void *runInputThread(void *arg) {
  fprintf(stderr,"running input thread\n");
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  char *line = NULL;
  size_t len = 0;
  ssize_t nread;
  
  FILE *stream = stdin;
  
  while ((nread = getline(&line, &len, stream)) != -1) {
    addPosition(threadContext->b, 1);
  }
  
  threadContext->b->inputComplete = 1;
  fprintf(stderr,"input thread complete\n");
  
  free(line);

  return NULL;
}
   
 

static void *runVerifyThread(void *arg)
{
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  //    fprintf(stderr,"running thread %zd\n", threadContext->id);
  while (threadContext->pleaseStop == 0) {

    size_t pos;
    if (isNextPosition(threadContext->b, &pos) == 1) {
      fprintf(stderr,"%lf [%zd] %zd, num %zd\n", timedouble(), threadContext->id, pos, threadContext->b->numPositions);
      //      usleep(300);
    } else {
      usleep(1000);
    }
    
  }
  //  fprintf(stderr,"*%zd* is finished %zd\n", threadContext->id, threadContext->pleaseStop);
  return NULL;
}

void setupVerificationThreads(blockVerifyType *b) {
  fprintf(stderr,"*info* setup %zd threads\n", b->threads);

  // add one for the input thread
  CALLOC(pt, b->threads + 1, sizeof(pthread_t)); 
  CALLOC(threadContext, b->threads + 1, sizeof(threadInfoType));
  if (pthread_mutex_init(&mutex, NULL) != 0) {
    fprintf(stderr,"*error* can't setup lock\n");exit(1);
  }

  for (size_t i = 1; i <= b->threads; i++) {
    //    fprintf(stderr,"setting thread %zd\n", i);
    threadContext[i].id = i;
    threadContext[i].pleaseStop = 0;
    threadContext[i].b = b;

    if (pthread_create(&(pt[i]), NULL, runVerifyThread, &(threadContext[i]))) {
      fprintf(stderr,"*error* can't create a thread\n");
      exit(-1);
    }
  }
  b->isSetup = 1;

  b->nextpos = 0;
  b->numPositions = 0;
}

void setupInputThread(blockVerifyType *b) {
  threadContext[0].id = 0;
  threadContext[0].pleaseStop = 0;
  threadContext[0].b = b;
  b->inputComplete = 0;
  if (pthread_create(&(pt[0]), NULL, runInputThread, &(threadContext[0]))) {
    fprintf(stderr,"*error* can't create a thread\n");
    exit(-1);
  }
}
  

blockVerifyType blockVerifySetup(size_t threads, size_t type, size_t o_direct, size_t batchsize) {
  blockVerifyType b;
  b.threads = threads;
  b.type = type;
  b.o_direct = o_direct;
  if (batchsize < 1) batchsize = 1;
  b.batchsize = batchsize;
  b.isSetup = 0;
  b.inQueue = 0;

  return b;
}



void blockVerifyStop(blockVerifyType *b) {
  for (size_t i = 1; i <= b->threads; i++) {
    threadContext[i].pleaseStop = 1;
  }
}

void blockVerifyFinish(blockVerifyType *b) {
  for (size_t i = 1; i <= b->threads; i++) {
    pthread_join(pt[i], NULL);
  }
  pthread_join(pt[0], NULL);
  free(pt); pt = NULL;
  free(threadContext); threadContext = NULL;
  pthread_mutex_destroy(&mutex);
}


int main() {
  blockVerifyType b = blockVerifySetup(16, 0, 0, 0);

  setupVerificationThreads(&b); // start running, initially no data, so they sleep

  setupInputThread(&b);

  // once all IO processed
  while ((b.inputComplete == 0) || (b.nextpos < b.numPositions)) {
    usleep(1000); 
  }
  blockVerifyStop(&b); // get the threads to return
  blockVerifyFinish(&b); // join and continue

  exit(0);
}
