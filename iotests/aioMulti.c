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
#include "positions.h"
#include "utils.h"

int keepRunning = 1;
int verbose = 0;
int flushEvery = 0;
double MAXRAM = 2L*1024*1024*1024 ; // 2GB
int tripleX = 0;

typedef struct {
  size_t fd;
  size_t id;
  size_t qd;
  size_t max;
  float readRatio;
  size_t total;
} threadInfoType;

static size_t blockSize = 65536;

volatile int ready = 0;
double startTime = 0;

void intHandler(int d) {
  fprintf(stderr,"got signal\n");
  keepRunning = 0;

}

static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  //  fprintf(stderr,"id: %zd\n", threadContext->id);
  
  size_t bdsize = blockDeviceSizeFromFD(threadContext->fd);
  //  fprintf(stderr,"[thread %zd] bdSize %zd (%.2lf GiB)\n", threadContext->id, bdsize, TOGiB(bdsize));

    
  size_t maxBlocks = bdsize / blockSize;
  if (bdsize > 1024L*1024L*1024*1024L) { // if over 1TiB 
    maxBlocks = 1024L*1024L*1024*1024L / blockSize;
  }
  double RAM = MAXRAM / threadContext->max;
  //  fprintf(stderr,"max ram %lf\n", RAM);
  size_t positionsNum = (RAM / sizeof(positionType)) + 1;
  if (positionsNum > maxBlocks) positionsNum = maxBlocks;
  //  fprintf(stderr,"max positions %zd\n", positionsNum);
  fprintf(stderr,"*info* %.1lf GiB in thread %zd (%.1lf GiB total)/ %zd, bdsize = %zd, positions %zd (%.1lf GiB covered)\n", TOGiB(RAM), threadContext->id, TOGiB(MAXRAM), threadContext->max, blockSize, positionsNum, TOGiB(blockSize * positionsNum));
  positionType *positions = createPositions(positionsNum);
  
  int fdA[1];
  fdA[0] = threadContext->fd;
  //    fprintf(stderr,"fd %d\n", fdA[0]);
  setupPositions(positions, &positionsNum, fdA, 1, bdsize, 1, threadContext->readRatio, blockSize, blockSize, blockSize, 0, 0, 0, bdsize, 0, NULL);
  
  
  char *randomBuffer = aligned_alloc(blockSize, blockSize); if (!randomBuffer) {fprintf(stderr,"oom!\n");exit(-1);}
  generateRandomBuffer(randomBuffer, blockSize);
  
  size_t ios, trb, twb;
  ready++;
  while (ready != threadContext->max && keepRunning) {
    usleep(100);
  }
  if (threadContext->id == 0) {
    startTime = timedouble();
  }
  //  fprintf(stderr,"go(%zd)\n",threadContext->id);
  threadContext->total = aioMultiplePositions(positions, positionsNum, 100, threadContext->qd, 0, 0, NULL, NULL, randomBuffer, blockSize, blockSize, &ios, &trb, &twb, 0);

  free(randomBuffer);
  free(positions);


  return NULL;
}




void startThreads(size_t *fdArray, size_t num, size_t qd, float rr) {
  
  pthread_t *pt;
  CALLOC(pt, num, sizeof(pthread_t));
  assert(pt);

  threadInfoType *threadContext;
  CALLOC(threadContext, num, sizeof(threadInfoType));
  assert(threadContext);

  for (size_t i = 0; i < num; i++) {
    threadContext[i].fd = fdArray[i];
    threadContext[i].id = i;
    threadContext[i].readRatio = rr;
    threadContext[i].qd = qd;
    threadContext[i].max = num;
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }

  for (size_t i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
  }

  double elapsed = timedouble() - startTime;
  size_t sum = 0;
  for (size_t i = 0; i <num;i++) {
    sum += threadContext[i].total;
  }
  fprintf(stderr,"*info* summary: %zd devices, readRatio %.1lf, total bytes %zd, %.1lf secs, %.1lf GiB/s (%.0f MiB/s/device)\n", num, rr, sum, elapsed, TOGiB(sum) / elapsed, TOMiB(sum) / elapsed/num);
  free(threadContext);
  free(pt);
}


double readRatio = 1;

int handle_args(int argc, char *argv[]) {

  if (argc <= 1) {
    fprintf(stderr,"./aioMulti [-R ramInGiB] [-r (default)] [-w] ...devices...\n");
    exit(-1);
  } else {
    int opt;
    while ((opt = getopt(argc, argv, "rwR:XV")) != -1) {
      switch (opt) {
      case 'V':
	verbose++;
	break;
      case 'R':
	MAXRAM = atoi(optarg) * 1024L * 1024L * 1024L;
	break;
      case 'r':
	readRatio = 1;
	break;
      case 'w':
	readRatio = 0;
	break;
      case 'X':
	tripleX++;
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

  MAXRAM =totalRAM() /2 ;

  int opt = handle_args(argc, argv);

  size_t *fdArray;
  CALLOC(fdArray, argc+1, sizeof(size_t));
  int fdCount = 0;
  for (size_t i = opt; i < argc; i++) {
    // add device
    int fd = 0;
    if (readRatio < 1) { // writes
      fd = open(argv[i], O_RDWR | O_DIRECT | O_TRUNC | O_EXCL);
    } else { // only reads
      if (tripleX >= 3) {
	fd = open(argv[i], O_RDONLY | O_DIRECT);
      } else {
	fd = open(argv[i], O_RDONLY | O_DIRECT | O_EXCL);
      }
    }
	    
    //	fprintf(stderr,"%s\n", argv[i]);
    if (fd <= 0) {
      perror(argv[i]);
      exit(-1);
    }
    fdArray[fdCount++] = fd;
  }

  const size_t qd = 30000 / fdCount;
  fprintf(stderr,"*info* maxRam %.1lf GiB, readRatio %.1f, %d devices, each with qd=%zd (30,000/%d)\n", TOGiB(MAXRAM), readRatio, fdCount, qd, fdCount);
  
  startThreads(fdArray, fdCount, qd, readRatio);

  free(fdArray);

  // create numDevices threads, call aioOperations per thread/device

  return 0;
}

