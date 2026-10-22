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

extern int keepRunning;

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
  double elapsed;
} threadInfoType;



int verifyPosition(positionType *p, const char *randomBuffer, char *buf, size_t len, size_t pos) {
  assert (p->dev->fd > 0);
  //  fprintf(stdout,"%s %d %zd\n", p->dev->devicename, p->dev->fd, pos);
  ssize_t ret = pread(p->dev->fd, buf, len, pos); // use pread as it's thread safe as you pass in the fd, size and offset
  if (ret == -1) {
    fprintf(stderr,"*error* seeking to pos %zd: ", pos);
    perror("pread");
    return -1;
  } else if (ret != len) {
    fprintf(stderr,"*error* position %zd, wrong len %zd instead of %zd\n", pos, ret, len);
    return -2;
  } else {
    if (memcmp(randomBuffer, buf, len) == 0) {
      return 0;
    }
  }

  int diff = 0;
  for (size_t j = 0; j< len ;j++) {
    if (buf[j] != randomBuffer[j]) {
      //      char s1[4097], s2[4097];
      //      strncpy(s1, buf, 4096);
      //      strncpy(s2, randomBuffer, 4096);
      //      s1[len+1] = 0;
      //      s2[len+1] = 0;
      if (diff < 10) {
	fprintf(stderr,"*error* block difference at position %zd in block (size %zd, div block %zd, remainder %zd)\n"
	      " different starting pos %zd, should be: char %d '%c' and %d '%c'\n", j, len, pos / len, pos % len, j, (int)buf[j], buf[j], (int)randomBuffer[j], randomBuffer[j]);
	fflush(stderr);
      }
      diff++;
      //      break;
    }
  }
  fprintf(stderr,"total diff %d\n", diff);
  return -3;
}


static void *runThread(void *arg) {

  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  char *buf = NULL;
  CALLOC(buf, 1, threadContext->blocksize);

  //    fprintf(stderr,"*info* id %d, [%ld, %ld),.... \n", threadContext->id, threadContext->startInc, threadContext->endExc);
  positionType *positions = threadContext->positions;
  for (size_t i = threadContext->startInc; i < threadContext->endExc; i++) {
    if (!keepRunning) {break;}
    if (positions[i].action == 'W' && positions[i].success) {
      threadContext->bytesRead += positions[i].len;
      //      double start = timeAsDouble();
      int ret = verifyPosition(&positions[i], threadContext->randomBuffer, buf, positions[i].len, positions[i].pos);

      if (ret !=0) {
	int ret2 = verifyPosition(&positions[i], threadContext->randomBuffer, buf, positions[i].len, positions[i].pos);
	if (ret2 == 0) {
	  fprintf(stderr,"*info* !surprise! the read worked again the second time!\n");
	}
      }
//      threadContext->elapsed = timeAsDouble() - start;
      switch (ret) {
      case 0: threadContext->correct++; break;
      case -1: threadContext->ioerrors++; break;
      case -2: threadContext->lenerrors++; break;
      case -3: threadContext->incorrect++; break;
      default:
	break;
      }
      //      if (ret != 0) {
      //	fprintf(stderr,"*info* result %d, time taken %.4lf\n", ret, threadContext->elapsed);
      //      }
    }
  }
  free(buf);

  return NULL;
}



  
/**
 *
 *
 */
int verifyPositions(positionType *positions, size_t numPositions,char *randomBuffer, size_t threads, long seed, size_t blocksize, size_t *correct, size_t *incorrect, size_t *ioerrors, size_t *lenerrors) {

  fprintf(stderr,"*info* starting verify...\n");
  
  pthread_t *pt = NULL;
  CALLOC(pt, threads, sizeof(pthread_t));
  threadInfoType *threadContext;
  CALLOC(threadContext, threads, sizeof(threadInfoType));

  double start = timeAsDouble();
    
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
    threadContext[i].elapsed = 0;

    if (pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]))) {
      fprintf(stderr,"*error* can't create a thread\n");
      exit(-1);
    }
  }

  for (size_t i = 0; i < threads; i++) {
    pthread_join(pt[i], NULL);
  }

  double elapsed = timeAsDouble() - start;

  size_t tr = 0;
  for (size_t i = 0; i < threads; i++) {
    (*correct) += threadContext[i].correct;
    (*incorrect) += threadContext[i].incorrect;
    (*ioerrors) += threadContext[i].ioerrors;
    (*lenerrors) += threadContext[i].lenerrors;
    tr += threadContext[i].bytesRead;
  }
  size_t ops = (*correct) + (*incorrect) + (*ioerrors) + (*lenerrors);

  fprintf(stderr,"*info* verify: %zd ops, %.1lf GiB, %.1lf s (t=%zd), %.1lf MiB/s\n", ops, TOGiB(tr), elapsed, threads, TOMiB(tr)/elapsed);

  free(pt);
  free(threadContext);

  return (*ioerrors) + (*lenerrors) + (*incorrect);
}


  

