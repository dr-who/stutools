#define _XOPEN_SOURCE 500

#include "workQueue.h"
#include "utils.h"

#include <stdlib.h>
#include <malloc.h>
#include <unistd.h>
#include <assert.h>

#include <pthread.h>

#include <string.h>


void workQueueInit(workQueueType *queue, const size_t size) {
  queue->allocsz = size;
  queue->sz = 0;
  queue->head = 0;
  queue->finished = 0;
  queue->tail = 0;
  queue->startTime = 0;
  queue->actions = malloc(size * sizeof(workQueueActionType*)); assert(queue->actions);
  if (pthread_mutex_init(&queue->lock, NULL) != 0) {
    printf("\n mutex init has failed\n"); 
    exit(1);
  }
}

int workQueuePush(workQueueType *queue, workQueueActionType *action) {
  int ret = 0;
  pthread_mutex_lock(&queue->lock); 

  if (queue->sz < queue->allocsz-1) {
    queue->actions[queue->head++] = action;
    if (queue->head >= queue->allocsz) {
      queue->head = 0;
    }
    queue->sz++;
    if (queue->startTime == 0) {
      queue->startTime = timedouble();
    }
  } else {
    ret = 1;
    fprintf(stderr,"*warning* not adding, full %zd\n", queue->sz);
  }

  pthread_mutex_unlock(&queue->lock);
  return ret;
}

workQueueActionType *workQueuePop(workQueueType *queue) {
  workQueueActionType *ret = NULL;
  if (pthread_mutex_trylock(&queue->lock) == 0) {
    if (queue->tail != queue->head) {
      ret = queue->actions[queue->tail];
      queue->tail++;
      if (queue->tail >= queue->allocsz) {
	queue->tail = 0;
      }
      queue->sz--;
      queue->finished++;
      queue->sizeSum += ret->size;
    }
    pthread_mutex_unlock(&queue->lock);
  }
  return ret;
}


void workQueueFree(workQueueType *queue) {
  queue->sz = 0;
  if (queue->actions) free(queue->actions);
  queue->actions = NULL;
  pthread_mutex_destroy(&queue->lock);
}


size_t workQueueNum(workQueueType *queue) {
  pthread_mutex_lock(&queue->lock);

  size_t sz = queue->sz;
  pthread_mutex_unlock(&queue->lock);
  return sz;
}


size_t workQueueFinished(workQueueType *queue) {
  pthread_mutex_lock(&queue->lock);

  size_t sz = queue->finished;
  pthread_mutex_unlock(&queue->lock);
  return sz;
}


size_t workQueueFinishedSize(workQueueType *queue) {
  pthread_mutex_lock(&queue->lock);

  size_t sz = queue->sizeSum;
  pthread_mutex_unlock(&queue->lock);
  return sz;
}


