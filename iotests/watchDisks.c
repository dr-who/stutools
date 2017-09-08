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

size_t  exitAfterSeconds = -1; // default timeout
double limitToGB = 0.02; // 10MiB

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
  int fd = open(threadContext->path, O_RDONLY | O_DIRECT); // may use O_DIRECT if required, although running for a decent amount of time is illuminating
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //  fprintf(stderr,"opened %s\n", threadContext->path);

  // do stuff
  //  size_t maxread = ((size_t)(1024*1024*1024 * limitToGB) >> 10) << 10; // size needs to be a multiple of 1k;
  //  fprintf(stderr,"fd %d, max read %zd\n", fd, maxread);
  //  char *buffer = aligned_alloc(65536, maxread); if (!buffer) {fprintf(stderr,"OOM\n");exit(1);}

  if (lseek(fd, threadContext->position, SEEK_SET) == -1) {
    perror("cannot seek");
  }
  //  fprintf(stderr,"%s to position %zd\n", threadContext->path, threadContext->position);
  
  ssize_t rb = read(fd, threadContext->data, threadContext->datalen);
  if (rb < 0) {
    perror("runThread\n");
  } else {
    //    fprintf(stderr,"."); fflush(stderr);
  }
  if (rb != threadContext->datalen) {
    fprintf(stderr,"eek!\n");
  }
  threadContext->datalen = rb;
  size_t checksum = 0;
  for (size_t i = 0; i < rb; i++) {
    checksum += threadContext->data[i];
  }
    
  threadContext->checksum = checksum;

  close(fd);
  return NULL;
}

size_t countDiff(volatile char *d1, volatile char *d2, size_t len) {
  size_t diff = 0;
  for (size_t i = 0; i < len; i++) {
    if (d1[i] != d2[i]) {
      diff++;
    }
  }
  return diff;
}

void startThreads(int argc, char *argv[], int index) {
  if (argc - index > 0) {
    size_t threads = argc - index;
    pthread_t *pt = (pthread_t*) calloc((size_t) threads, sizeof(pthread_t));
    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}

    threadInfoType *threadContext, *nextContext;
    CALLOC(threadContext, threads, sizeof(threadInfoType));
    CALLOC(nextContext, threads, sizeof(threadInfoType));

    const size_t maxread = ((size_t)(1024*1024*1024 * limitToGB) >> 10) << 10; // size needs to be a multiple of 1k;

    // grab the first one
    for (size_t i = 0; i < threads; i++) {
      if (argv[i + index][0] != '-') {
	threadContext[i].threadid = i;
	threadContext[i].position = 0;
	threadContext[i].datalen = maxread;
	threadContext[i].data = aligned_alloc(65536, maxread); if (!threadContext[i].data) {fprintf(stderr,"OOM\n");exit(1);}

	threadContext[i].path = argv[i + index];
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }

    for (size_t i = 0; i < threads; i++) {
      if (argv[i + index][0] != '-') {
	pthread_join(pt[i], NULL);
	fprintf(stderr,"path %s, position %zd, watching up to = %zd bytes (%.2lf GiB)\n", threadContext[i].path, threadContext[i].position, threadContext[i].datalen, TOGiB(threadContext[i].datalen));
      }
    }


    double start = timedouble();

    while (((timedouble() - start) < exitAfterSeconds) && keepRunning) {
      // grab the next one
      for (size_t i = 0; i < threads; i++) {
	if (argv[i + index][0] != '-') {
	  nextContext[i].threadid = i;
	  nextContext[i].position = 0;
	  nextContext[i].datalen = maxread;
	  if (!nextContext[i].data) {
	    nextContext[i].data = aligned_alloc(65536, maxread); if (!nextContext[i].data) {fprintf(stderr,"OOM\n");exit(1);}
	  }
	  
	  nextContext[i].path = argv[i + index];
	  pthread_create(&(pt[i]), NULL, runThread, &(nextContext[i]));
	}
      }
      
      for (size_t i = 0; i < threads; i++) {
	size_t diff = 0;
	if (argv[i + index][0] != '-') {
	  pthread_join(pt[i], NULL);
	  //	  fprintf(stderr,"2 path %s, position %zd, sz = %ld, checksum = %zd\n", nextContext[i].path, nextContext[i].position, nextContext[i].datalen, nextContext[i].checksum);

	  volatile char *d1 = nextContext[i].data;
	  volatile char *d2 = threadContext[i].data;
	  if ((diff = countDiff(d1, d2, nextContext[i].datalen))) {
	    fprintf(stderr,"%s (%zd)", nextContext[i].path, diff);
	    size_t lastpos = 0, printed = 0;
	    for (size_t j = 0; j < nextContext[i].datalen; j++) {
	      if (d1[j] != d2[j]) {
		if (printed++ < 10) {
		  fprintf(stderr,"[%2x %c]", (unsigned char)d1[j], (unsigned char)d1[j]);
		} else if (printed == 10) {
		  fprintf(stderr,".....\n");
		}
		lastpos = j;
	      }
	    }
	    /*	    if (diff) {
	      fprintf(stderr,"\n");
	      }*/
	    //	    fprintf(stderr,"....DIFFERENT.... %zd ... %zd (count = %zd, pos = %zd)\n", threadContext[i].checksum, nextContext[i].checksum, diff, lastpos);
	    fprintf(stderr,"\t\tdiff = %zd, pos = %zd (%.4lf GiB)\n", diff, lastpos, TOGiB(lastpos));
	    threadContext[i].checksum = nextContext[i].checksum;
	    for (size_t k=0; k < nextContext[i].datalen; k++) {
	      threadContext[i].data[k] = nextContext[i].data[k];
	    }
	  }
	}
      }
      
      sleep(1);
    }
    //output info

    for (size_t i = 0; i < threads; i++) {
      if (argv[i + index][0] != '-') {
	free(threadContext[i].data);
	free(nextContext[i].data);
      }
    }
    
    free(threadContext);
    free(nextContext);
    free(pt);
  }
}

int handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "G:t:")) != -1) {
    switch (opt) {
    case 'G':
      limitToGB = atof(optarg);
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
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"*info* number of disks %d, timeout=%zd, limit=%.2lf GiB\n", argc - index, (ssize_t) exitAfterSeconds, limitToGB);
     
  startThreads(argc, argv, index);
  return 0;
}
