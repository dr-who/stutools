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


int keeprunning = 1;
int useDirect = 1;
float benchmarkTime = 2;
size_t atLeastN = 150;

typedef struct {
  int threadid;
  char *path;
  size_t total;
  int allrunning;
  //  double speedmb;
  logSpeedType logspeed;
} threadInfoType;


void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keeprunning = 0;
}
static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  //  fprintf(stderr,"opening... '%s'\n", threadContext->path);
  int fd = open(threadContext->path, O_EXCL | O_WRONLY | (useDirect ? O_DIRECT : 0));
  if (fd < 0) {
    perror(threadContext->path);
    return NULL;
  }
  //  fprintf(stderr,"opened %s\n", threadContext->path);
#define BUFSIZE (1024*1024)
  void *buf = NULL;
  if (posix_memalign(&buf, 4096, BUFSIZE)) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }	
  memset(buf, 0x00, BUFSIZE);
  int wbytes = 0;
  size_t lastg = 0;
  logSpeedType *l = &(threadContext->logspeed);
  logSpeedInit(l);
  
  while (((wbytes = write(fd, buf, BUFSIZE)) > 0) && keeprunning) {
    threadContext->total += wbytes;
    logSpeedAdd(l, wbytes);
    if (threadContext->total >> 30 != lastg) {
      lastg = threadContext->total >>30;
    }
    if ((logSpeedTime(l) >= benchmarkTime) && (logSpeedN(l) >= atLeastN) && threadContext->allrunning) break; // at least 100 data points
  }
  if (wbytes < 0) {
    perror("weird problem");
  }
  close(fd);
  free(buf);
  return NULL;
}

size_t benchmark(threadInfoType *threadContext, const int num, volatile size_t running[]) {
    pthread_t *pt = calloc(num, sizeof(pthread_t));    if (pt==NULL) {fprintf(stderr, "OOM(pt): \n");exit(-1);}


    //    fprintf(stderr,"\n\n");
    for (size_t i = 0; i < num; i++) {
      if (running[i]) {
	//	fprintf(stderr,"running=%zd\n",i);
	threadContext[i].total = 0;
	threadContext[i].allrunning = 0;
	logSpeedInit(&threadContext[i].logspeed);
	//	fprintf(stderr, "writing to %s \n", threadContext[i].path);
	pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
      }
    }

    // after the threads have started, check until they're all actively writing
    
    while (1) {
      int allrunning = 1;
      for (size_t i = 0; i < num; i++) {
	if (running[i]) {
	  if (logSpeedN(&threadContext[i].logspeed) < 50) {
	    allrunning = 0;
	    break;
	  }
	}
      }
      if (allrunning == 0) {
	sleep(0.1); // wait for a bit then try again
      } else {
	break;
      }
    }

    for (size_t i = 0; i < num; i++) {
      if (running[i]) {
	logSpeedReset(&threadContext[i].logspeed);
	threadContext[i].allrunning = 1;
      }
    }

    // wait for complete

    
    size_t allbytes = 0;
    size_t speedmb = 0;
    for (size_t i = 0; i < num; i++) {
      if (running[i]) {
	pthread_join(pt[i], NULL);
	allbytes += threadContext[i].total;
	speedmb += (logSpeedMedian(&threadContext[i].logspeed) / 1024.0 / 1024); // sum the MB per drive
      }
    }
    //    size_t speedmb =  (size_t) ((allbytes/1024.0/1024) / elapsedtime);
    
    free(pt);
    return speedmb;
}



threadInfoType *gatherDrives(int argc, char *argv[], int *num) {
    threadInfoType *threadContext = (threadInfoType*) calloc(argc, sizeof(threadInfoType));
    if (threadContext == NULL) {fprintf(stderr,"OOM(threadContext): \n");exit(-1);}
    threadInfoType *ret = threadContext;
    
    *num = 0;
    if (argc > 0) {
      
      for (size_t i = 1; i < argc; i++) {
	if (argv[i][0] != '-') {
	  threadContext->threadid = *num;
	  *num = (*num) + 1;
	  threadContext->path = argv[i];
	  threadContext->total = 0;
	  fprintf(stderr,"drive %d: %s\n", *num, threadContext->path);
	  threadContext++;
	}
      }
    }
    return ret;
}

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "dDfFS")) != -1) {
    switch (opt) {
    case 'd':
      useDirect = 1;
      break;
    case 'D':
      useDirect = 0;
      break;
    case 'f':
      benchmarkTime = 1;
        break;
    case 'F':
      benchmarkTime = 0.3;
        break;
    case 'S':
      benchmarkTime = 3;
      break;
    } 
  }

}


void nSquareTest(threadInfoType *t, const int num) {

  size_t values[num][num];
  volatile size_t *running = calloc(100, sizeof(size_t));

  fprintf(stdout,"\nThe performance table is in MB/s:\n     ");
  for (size_t i = 0; i < num; i++) {
    fprintf(stdout,"%4zd ", i+1);
  }
  fprintf(stdout,"\n");
  
  for (size_t j = 0; j < num; j++) {
    fprintf(stdout,"%4zd ", j+1);
    for (size_t i = 0; i < num; i++) {

      for (size_t z = 0; z < num; z++) {
	running[z] = 0;
      }

      running[i] = 1;
      running[j] = 1;

      // do test
      size_t speedmb = 0;
      if (i >= j) {
	speedmb = benchmark(t, num, running);
	values[j][i] = speedmb;
      } else {
	values[j][i] = values[i][j];
      }
	
      fprintf(stdout,"%4zd ", values[j][i]);
      fflush(stdout);
    }
    fprintf(stdout,"\n");
  }

  fprintf(stdout,"\nAnalysing dependencies:\n");
  for (size_t i = 0; i < num; i++) {
    for (size_t j = 0 ; j < num; j++) if (i != j) {
      size_t one = values[i][i];
      size_t two = values[j][j];
      size_t couldbe = one + two;
      int diff = (values[j][i] - couldbe);
      if (diff < 0) diff = -diff;

      if (diff / 1.0 / couldbe > 0.2) {
	fprintf(stdout,"  Performance of %zd and %zd look dependent\n", i+1, j+1);
      } 
    }
  }
    

}

int main(int argc, char *argv[]) {
  handle_args(argc, argv);

  int num = 0;
  threadInfoType *t = gatherDrives(argc, argv, &num);

  if (num > 0) {
    fprintf(stderr,"timeout: %.1f seconds, direct=%d, at least N samples=%zd\n", benchmarkTime, useDirect, atLeastN);
    nSquareTest(t, num);
  }
  
  return 0;
}





