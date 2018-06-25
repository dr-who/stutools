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

int verbose = 0;
int keepRunning = 1;


typedef struct {
  int id;
  size_t startInc;
  size_t endExc;
  size_t correct;
  size_t incorrect;
  size_t ioerrors;
  size_t lenerrors;
  long seed;
  size_t blocksize;
  positionType *positions;
  char *randomBuffer;
  size_t bytesRead;
} threadInfoType;



int verifyPosition(positionType *p, char *randomBuffer, char *buf, size_t len, size_t pos) {
  assert (p->dev->fd > 0);
  //  fprintf(stdout,"%s %d %zd\n", p->dev->devicename, p->dev->fd, pos);
  ssize_t ret = pread(p->dev->fd, buf, len, pos); // use pread as it's thread safe as you pass in the fd, size and offset
  if (ret == -1) {
    fprintf(stderr,"*error* seeking to pos %zd: ", pos);
    perror("pread");
    return -1;
  } else if (ret != len) {
    fprintf(stderr,"*error* wrong len %ld instead of %zd\n", ret, len);
    return -2;
  } else {
    if (memcmp(randomBuffer, buf, len) == 0) {
      return 0;
    }
  }
    
  fprintf(stderr,"*error* block difference at position %zd (div block %zd, remainder %zd)\n", pos, pos / len, pos % len);
  for (size_t j = 0; j< len ;j++) {
    if (buf[j] != randomBuffer[j]) {
      fprintf(stderr,"  byte different at %zd, '%c' '%c'\n", j, buf[j], randomBuffer[j]);
      break;
    }
  }
  return -3;
}


static void *runThread(void *arg) {

  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  char *buf = NULL;
  posix_memalign((void**)&buf, threadContext->blocksize, threadContext->blocksize); if (!buf) {fprintf(stderr,"oom!\n");exit(-1);}

  //    fprintf(stderr,"*info* id %d, [%ld, %ld),.... \n", threadContext->id, threadContext->startInc, threadContext->endExc);
  positionType *positions = threadContext->positions;
  for (size_t i = threadContext->startInc; i < threadContext->endExc; i++) {
    if (positions[i].action == 'R') {
      threadContext->bytesRead += positions[i].len;
      int ret = verifyPosition(&positions[i], threadContext->randomBuffer, buf, positions[i].len, positions[i].pos);
      switch (ret) {
      case 0: threadContext->correct++; break;
      case -1: threadContext->ioerrors++; break;
      case -2: threadContext->lenerrors++; break;
      case -3: threadContext->incorrect++; break;
      default:
	break;
      }
    }
  }

  free(buf);

  return NULL;
}



  

/**
 *
 *
 */
void verifyPositions(positionType *positions, size_t numPositions,char *randomBuffer, size_t threads, long seed, size_t blocksize, size_t *correct, size_t *incorrect, size_t *ioerrors, size_t *lenerrors) {
  pthread_t *pt = NULL;
  CALLOC(pt, threads, sizeof(pthread_t));
  threadInfoType *threadContext;
  CALLOC(threadContext, threads, sizeof(threadInfoType));


  double start = timedouble();
    
  for (size_t i =0 ; i < threads; i++) {
    threadContext[i].id = i;
    threadContext[i].randomBuffer = randomBuffer;
    threadContext[i].startInc = (size_t) (i*(numPositions * 1.0 / threads));
    threadContext[i].endExc =   (size_t) ((i+1)*(numPositions * 1.0 / threads));
    threadContext[i].correct = 0;
    threadContext[i].incorrect = 0;
    threadContext[i].ioerrors = 0;
    threadContext[i].lenerrors = 0;
    threadContext[i].seed = seed;
    threadContext[i].blocksize = blocksize;
    threadContext[i].positions = positions;
    threadContext[i].bytesRead = 0;
    
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }

  for (size_t i = 0; i < threads; i++) {
    pthread_join(pt[i], NULL);
  }

  double elapsed = timedouble() - start;

  size_t tr = 0;
  for (size_t i = 0; i < threads; i++) {
    (*correct) += threadContext[i].correct;
    (*incorrect) += threadContext[i].incorrect;
    (*ioerrors) += threadContext[i].ioerrors;
    (*lenerrors) += threadContext[i].lenerrors;
    tr += threadContext[i].bytesRead;
  }

  fprintf(stderr,"*info* %zd bytes (%.2lf GiB) verified in %.2lf seconds = %.1lf MiB/s\n", tr, TOGiB(tr), elapsed, TOMiB(tr)/elapsed);

  free(randomBuffer);
  free(pt);
  free(threadContext);
}


  

