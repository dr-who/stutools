#define _POSIX_C_SOURCE 200809L

/**
 * blockVerify.c
 *
 *
 */
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <assert.h>

#include "positions.h"
#include "devices.h"
#include "utils.h"
#include "blockVerify.h"

extern int keepRunning;

typedef struct {
  int fd;
  int id;
  int numThreads;
  size_t startInc;
  size_t endExc;
  size_t correct;
  size_t incorrect;
  size_t ioerrors;
  size_t lenerrors;
  positionContainer *pc;
  size_t bytesRead;
  size_t iocount;
  double elapsed;
} threadInfoType;

// sorting function, used by qsort
/*static int seedcompare(const void *p1, const void *p2)
{
  const positionType *pos1 = (positionType*)p1;
  const positionType *pos2 = (positionType*)p2;
  if (pos1->seed < pos2->seed) return -1;
  else if (pos1->seed > pos2->seed) return 1;
  else {
    if (pos1->pos < pos2->pos) return -1;
    else if (pos1->pos > pos2->pos) return 1;
    else return 0;
  }
  }*/



int verifyPosition(const int fd, const positionType *p, const char *randomBuffer, char *buf, size_t *diff) {
  const size_t pos = p->pos;
  const size_t len = p->len;

  assert(p->action == 'W');
  ssize_t ret = pread(fd, buf, len, pos); // use pread as it's thread safe as you pass in the fd, size and offset

  if (ret == -1) {
    fprintf(stderr,"*error* seeking to pos %zd: ", pos);
    perror("pread");
    return -1;
  } else if (ret != len) {
    fprintf(stderr,"*error* position %zd, wrong len %zd instead of %zd\n", pos, ret, len);
    return -2;
  } else {
    size_t *p1 = (size_t*)buf;
    size_t *p2 = (size_t*)randomBuffer;
    if (*p1 != *p2) {
      fprintf(stderr,"*error* encoded positions are wrong %zd (from disk) and %zd (at position %zd)\n", *p1, *p2, pos);
      return -3;
    }
    
    
    if (strncmp(buf+16, randomBuffer+16, p->len-16-2)==0) {
      return 0;
      // ok
    }

    if (*diff < 3) {
      size_t lines = 0;
      for (size_t i = 16; i < len; i++) {
	if (buf[i] != randomBuffer[i]) {
	  fprintf(stderr,"*error* difference at block[%zd] offset %zd, disk '%c', should be '%c'\n", pos, i, buf[i], randomBuffer[i]);
	  lines++;
	  if (lines>10) break;
	}
      }
      
      /*char s1[1000],s2[1000];
      memcpy(s1, buf+16, 80); s1[80]=0;
      memcpy(s2, randomBuffer+16, 80); s2[80]=0;
      fprintf(stderr,"*error* difference at block[%zd], len=%zd\n   disk:     %s\n   shouldbe: %s\n", pos, len, s1, s2);*/
    }
    *diff = (*diff)+1;
    
    return -3;
  }
}


static void *runThread(void *arg) {

  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  char *buf = NULL, *randombuf = NULL;
  CALLOC(randombuf, threadContext->pc->maxbs+1, 1);
  CALLOC(buf, threadContext->pc->maxbs+1, 1);

  //      fprintf(stderr,"*info* id %d, [%ld, %ld),.... \n", threadContext->id, threadContext->startInc, threadContext->endExc);
  size_t lastseed = -1;
  positionType *positions = threadContext->pc->positions;
  size_t diff = 0;
  

  for (size_t i = threadContext->startInc; i < threadContext->endExc; i++) {
    if (threadContext->id == threadContext->numThreads - 1) {
      // print progress
      size_t gap = threadContext->endExc - threadContext->startInc - 1;
      if (isatty(fileno(stderr))) {
	fprintf(stderr,"*progress* %.1lf %%\r", (i - threadContext->startInc) * 100.0 / gap);
	fflush(stderr);
      }
    }
    if (!keepRunning) {break;}
    if (positions[i].action == 'W' && positions[i].finishtime>0) {
      threadContext->bytesRead += positions[i].len;
      if (positions[i].seed != lastseed) {
	generateRandomBuffer(randombuf, threadContext->pc->maxbs, positions[i].seed);
	lastseed = positions[i].seed;
      }
	
      //      double start = timedouble();
      size_t pos = threadContext->pc->positions[i].pos;
      memcpy(randombuf, &pos, sizeof(size_t));
      int ret = verifyPosition(threadContext->fd, &threadContext->pc->positions[i], randombuf, buf, &diff);

      //      threadContext->elapsed = timedouble() - start;
      switch (ret) {
      case 0: threadContext->correct++; break;
      case -1: threadContext->ioerrors++; break;
      case -2: threadContext->lenerrors++; break;
      case -3: threadContext->incorrect++; break;
      default:
	break;
      }
      
      threadContext->iocount++;

    }
  }
  free(buf);
  free(randombuf);

  if (threadContext->id == 0) {
    if (isatty(fileno(stderr))) {
      fprintf(stderr,"\n");
    }
  }
  return NULL;
}



  
/**
 * Input is sorted
 *
 */
int verifyPositions(const int fd, positionContainer *pc, const size_t threads) {

  //  fprintf(stderr,"*info* sorting %zd\n", pc->sz);
  //  qsort(pc->positions, pc->sz, sizeof(positionType), seedcompare);
  keepRunning = 1;

  size_t num = pc->sz;

  pthread_t *pt = NULL;
  CALLOC(pt, threads, sizeof(pthread_t));
  threadInfoType *threadContext;
  CALLOC(threadContext, threads, sizeof(threadInfoType));

  double start = timedouble();
    
  for (size_t i =0 ; i < threads; i++) {
    threadContext[i].fd = fd;
    threadContext[i].id = i;
    threadContext[i].numThreads = threads;
    threadContext[i].startInc = (size_t) (i*(num * 1.0 / threads));
    threadContext[i].endExc =   (size_t) ((i+1)*(num * 1.0 / threads));
    threadContext[i].correct = 0;
    threadContext[i].incorrect = 0;
    threadContext[i].ioerrors = 0;
    threadContext[i].lenerrors = 0;
    threadContext[i].pc = pc;
    threadContext[i].bytesRead = 0;
    threadContext[i].iocount = 0;
    threadContext[i].elapsed = 0;

    //        fprintf(stderr,"*info* starting thread[%zd] in range [%zd, %zd)\n", i, threadContext[i].startInc, threadContext[i].endExc);
    
    if (pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]))) {
      fprintf(stderr,"*error* can't create a thread\n");
      exit(-1);
    }
  }

  for (size_t i = 0; i < threads; i++) {
    pthread_join(pt[i], NULL);
  }

  double elapsed = timedouble() - start;

  size_t tr = 0;
  size_t correct = 0, incorrect = 0, ioerrors = 0, lenerrors = 0, iocount = 0;
  
  for (size_t i = 0; i < threads; i++) {
    correct += threadContext[i].correct;
    incorrect += threadContext[i].incorrect;
    ioerrors += threadContext[i].ioerrors;
    lenerrors += threadContext[i].lenerrors;
    iocount += threadContext[i].iocount;
    
    tr += threadContext[i].bytesRead;
  }
  size_t ops = correct + incorrect + ioerrors + lenerrors;
  assert (ops == iocount);

  fprintf(stderr,"*info* verify: correct %zd, incorrect %zd, %zd ops (%.0lf IOPS), %.1lf GB, %.1lf s (threads=%zd), %.1lf MB/s\n", correct, incorrect, ops, ops/elapsed, TOGB(tr), elapsed, threads, TOMB(tr)/elapsed);

  free(pt);
  free(threadContext);

  return ioerrors + incorrect+ lenerrors;
}


  

