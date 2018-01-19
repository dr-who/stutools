#define _DEFAULT_SOURCE
#define _GNU_SOURCE
 
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <assert.h>

#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>

#include <pthread.h>

#include "aioRequests.h"
#include "positions.h"
#include "utils.h"

int keepRunning = 1;
int verbose = 0;
int flushEvery = 0;

typedef struct {
  size_t id;
  positionType *positions;
  size_t numpositions;
} threadInfoType;

static size_t blockSize = 65536*4;

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  //  fprintf(stderr,"id: %zd\n", threadContext->id);

  char *randomBuffer = aligned_alloc(blockSize, blockSize); if (!randomBuffer) {fprintf(stderr,"oom!\n");exit(-1);}
  generateRandomBuffer(randomBuffer, blockSize);

  size_t ios, trb, twb;
  size_t qd = 256;
  aioMultiplePositions(threadContext->positions, threadContext->numpositions, 10, qd, 0, 0, NULL, NULL, randomBuffer, blockSize, blockSize, &ios, &trb, &twb, 0);

  return NULL;
}




void startThreads(positionType **positions, size_t *numpositions, size_t num) {
  
  pthread_t *pt;
  CALLOC(pt, num, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num, sizeof(threadInfoType));

  for (size_t i = 0; i < num; i++) {
    threadContext[i].id = i;
    threadContext[i].positions = positions[i];
    threadContext[i].numpositions = numpositions[i];
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }

  for (size_t i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
  }
  free(threadContext);
  free(pt);
}


double readRatio = 0.5;

int handle_args(int argc, char *argv[]) {

  if (argc <= 1) {
    fprintf(stderr,"./aioMulti [-r] [-w] ...devices...\n");
    exit(-1);
  } else {
    int opt;
    while ((opt = getopt(argc, argv, "rw")) != -1) {
      switch (opt) {
      case 'r':
	readRatio += 0.5;
	break;
      case 'w':
	readRatio -= 0.5;
	break;
      default:
	break;
      }
    }
    return optind;
  }
}


int main(int argc, char *argv[]) {

  int opt = handle_args(argc, argv);

  size_t *fdArray;
  CALLOC(fdArray, 100, sizeof(size_t));
  int fdCount = 0;
  for (size_t i = opt; i < argc; i++) {
    // add device
    int fd = open(argv[i], O_RDWR | O_DIRECT | O_TRUNC | O_EXCL);
    assert(fd>=0);
    fdArray[fdCount++] = fd;
  }

  positionType **positions;
  size_t *positionsNum;
  CALLOC(positions, fdCount, sizeof(positionType*));
  CALLOC(positionsNum, fdCount, sizeof(size_t));


  for (size_t i = 0; i < fdCount; i++) {
    size_t bdsize = blockDeviceSizeFromFD(fdArray[i]);
    //    fprintf(stderr,"%zd: %zd\n", i, bdsize);

    positionsNum[i] = 10000000 + i;
    positions[i] = createPositions(positionsNum[i]);

    int fdA[1];
    fdA[0] = fdArray[i];
    //    fprintf(stderr,"fd %d\n", fdA[0]);
    setupPositions(positions[i], &(positionsNum[i]), fdA, 1, bdsize, 1, readRatio, blockSize, blockSize, blockSize, 0, 0, 1, bdsize, 0, NULL);
  }

  startThreads(positions, positionsNum, fdCount);

  // create numDevices threads, call aioOperations per thread/device

  return 0;
}

