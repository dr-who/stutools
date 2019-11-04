#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L
#define _XOPEN_SOURCE 500

#include "workQueue.h"
#include "utils.h"


#include <stdlib.h>
#include <malloc.h>
#include <assert.h>
#include <unistd.h>

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>


#include <pthread.h>
#include <string.h>

workQueueType wq;
size_t finished;
int verbose = 0;
int keepRunning = 1;

typedef struct {
  size_t threadid;
} threadInfoType;

void createAction(const char *filename, char *buf, size_t size) {
  int fd = open(filename, O_DIRECT | O_CREAT | O_TRUNC | O_RDWR, S_IRUSR | S_IWUSR);
  
  if (fd > 0) {
    size_t towrite = size;
    while (towrite > 0) {
      int written = write(fd, buf, size);
      if (written > 0) {
	towrite -= written;
      }
    }
    close(fd);
    if (verbose) fprintf(stderr,"*info* wrote file %s, size %zd\n", filename, size);
  } else {
    fprintf(stderr,"*error* didn't write %s\n", filename);
    exit(1);
  }
}


void* worker(void *arg) 
{
  threadInfoType *threadContext = (threadInfoType*)arg;
  char *buf = aligned_alloc(4096, 1024*1024*1);
  while (1) {
    
    workQueueActionType *action = workQueuePop(&wq);
    if (action) {

      size_t fin = workQueueFinished(&wq);
      if (fin % 1000 == 0) {
	double tm = timedouble() - wq.startTime;
	size_t sum = workQueueFinishedSize(&wq);
	fprintf(stderr,"*info* [thread %zd] [action %zd, '%s'], finished %zd (%.0lf files/second), %.1lf GiB (%.0lf MiB/s), %.1lf seconds\n", threadContext->threadid, action->id, action->payload, fin, fin/tm, TOGiB(sum), TOMiB(sum)/tm, tm);
      }
      
      switch(action->type) {
      case 'C': 
	//	fprintf(stderr,"[%zd] %c %s\n", action->id, action->type, action->payload);
	if (action->size < 1024*1024*1) {
	  createAction(action->payload, buf, action->size);
	} else {
	  fprintf(stderr,"*warning* big file ignored\n");
	}
	break;
      case 'D': 
	fprintf(stderr,"[%zd] theD %c %s\n", action->id, action->type, action->payload);
	break;
      }
      
      free(action->payload);
      free(action);
    } else {
      usleep(10000);
    }
  }
  free(buf);
  return NULL;
}


int main(int argc, char *argv[]) {

  int threads = 1;
  size_t blocksize = 360*1024;

  int opt = 0;
  while ((opt = getopt(argc, argv, "T:Vk:")) != -1) {
    switch (opt) {
    case 'k':
      blocksize = 1024 * atoi(optarg);
      break;
    case 'T':
      threads = atoi(optarg);
      break;
    case 'V':
      verbose = 1;
      break;
    }
  }
      
  
  workQueueInit(&wq, 10000);

  finished = 0;

  fprintf(stderr,"*info* number of threads %d, file size %zd\n", threads, blocksize);

  // setup worker threads
  pthread_t *tid = malloc(threads * sizeof(pthread_t));
  
  threadInfoType *threadinfo = malloc(threads * sizeof(threadInfoType));
  
  for (size_t i = 0; i < threads; i++){
    threadinfo[i].threadid = i;
    int error = pthread_create(&(tid[i]), NULL, &worker, &threadinfo[i]);
    if (error != 0) {
      printf("\nThread can't be created :[%s]", strerror(error)); 
    }
  }


  char s[1000];
  fprintf(stderr,"*info* making 100 top level directories\n");
  for (unsigned int id = 0; id < 100; id++) {
    sprintf(s,"%02x", id);
    mkdir(s, 0777);
  }
  fprintf(stderr,"*info* making 10000 two level directories\n");
  for (unsigned int id = 0; id < 10000; id++) {
    
    sprintf(s,"%02x/%02x", (((unsigned int)id)/100) % 100, ((unsigned int)id)%100);
    mkdir(s, 0777);
  }

  size_t id = 0;
  while (!finished) {
    if (workQueueNum(&wq) < 10000) { // queue up to 10,000 items at a time
      workQueueActionType *action = malloc(sizeof(workQueueActionType));

      action->type = 'C';
      sprintf(s,"%02x/%02x/%010zd", (((unsigned int)id)/100) % 100, ((unsigned int)id)%100, id);
      action->payload = strdup(s);
      action->id = id++;
      action->size = blocksize;
      if (workQueuePush(&wq, action) != 0) {
	free(action->payload);
	free(action);
      }
      //      fprintf(stderr,"*info* [%zd] num %zd\n", id, workQueueNum(&wq));
    } else {
      //      fprintf(stderr,"sleeping...%d in the queue\n", 1000);
      usleep(10000);
    }
  }
  

  workQueueFree(&wq);
}
