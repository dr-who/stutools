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
#include <math.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <assert.h>

#include "utils.h"
#include "positions.h"
#include "blockVerify.h"


typedef struct {
  size_t seed;
  size_t merge; 
  size_t randomize;
  size_t o_direct;
  size_t threads;
  size_t isSetup;
  size_t inQueue;

  size_t numPositions;
  size_t maxAllocPositions;
  size_t nextpos;
  
  size_t inputComplete;
  size_t minbs, maxbs;
  
  positionType *p;
  jobType *j;

  double start, finish;
  size_t onlycheckfirst;
} blockVerifyType;


typedef struct {
  size_t id;
  size_t pleaseStop;
  size_t processed;
  size_t correct;
  size_t wrong;

  size_t bytesProcessed;

  blockVerifyType *b;
} threadInfoType;

pthread_t *pt;
threadInfoType *threadContext;
pthread_mutex_t mutex;

int verbose;
int keepRunning;

void addPosition(blockVerifyType *b, positionType *p) {
  assert(b);
  pthread_mutex_lock(&mutex);
  b->numPositions = b->numPositions+1;

  if (b->numPositions >= b->maxAllocPositions) {
    b->maxAllocPositions += 5000;
    b->p = realloc(b->p, (b->maxAllocPositions) * sizeof(positionType));
    //    fprintf(stderr,"*realloc to %zd\n", b->maxAllocPositions);
  }

  memcpy(&b->p[b->numPositions-1], p, sizeof(positionType));
  if (p->len > b->maxbs) b->maxbs = p->len;
  if (p->len < b->minbs) b->minbs = p->len;
  //  fprintf(stdout,"*added* %zd\n", b->p[b->numPositions-1].pos);
  
  //  b->p[b->numPositions-1].pos = pos;
  assert(b->p);
  pthread_mutex_unlock(&mutex);
}
  
  


positionType * isNextPosition(blockVerifyType *b) {
  positionType *ret = NULL;
  pthread_mutex_lock(&mutex);
  if (b && (b->nextpos < b->numPositions)) {
    ret = &b->p[b->nextpos];
    //    fprintf(stderr, "---%zd\n", nextpos);
    b->nextpos++;
  }
  pthread_mutex_unlock(&mutex);
  return ret;
}
  
static void *runInputThread(void *arg) {
  fprintf(stderr,"running input thread\n");
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args

  char *line = NULL;
  //  size_t len = 0;
  //  ssize_t nread;

  size_t lineswitherrors = 0;
  positionContainer pc;
  while (1) {
    positionContainerLoadLines(&pc, stdin, 1, &lineswitherrors);
    if (pc.sz == 0) break;
    //    fprintf(stderr,"*data* %zd\n", pc.positions[0].pos);
    addPosition(threadContext->b, &pc.positions[0]);
  }
  
  //  while ((nread = getline(&line, &len, stream)) != -1) {
  //  }
  
  fprintf(stderr,"*info* input complete\n");
  fprintf(stderr,"should i randomize %zd\n", threadContext->b->randomize);
  if (threadContext->b->randomize) {
    positionsRandomize(threadContext->b->p, threadContext->b->numPositions, threadContext->b->seed);
  } else {
    positionsSortPositions(threadContext->b->p, threadContext->b->numPositions);
  }
  threadContext->b->inputComplete = 1;
  
  free(line);

  return NULL;
}
   
 

static void *runVerifyThread(void *arg)
{
  threadInfoType *threadContext = (threadInfoType*)arg; // grab the thread threadContext args
  //    fprintf(stderr,"running thread %zd\n", threadContext->id);

  char *buf = NULL, *randombuf = NULL;
  //  fprintf(stderr,"max len = %zd\n", threadContext->b->maxbs);
  assert(threadContext->b->maxbs < 100*1024*1024);
  CALLOC(randombuf, 100*1024*1024+1, 1);
  CALLOC(buf, 100*1024*1024+1, 1);

  int lastdevid = -1, lastseed = -1, fd = -1;
  while (threadContext->pleaseStop == 0) {
    // process if:
    //   if merge and input complete
    //   or
    //   if !merge
    size_t go = 0;
    if ((threadContext->b->merge == 1) && threadContext->b->inputComplete == 1) {
      go = 1;
    }
    if (threadContext->b->merge == 0) {
      go = 1;
    }

    positionType *pp = NULL;
    if (go && ((pp = isNextPosition(threadContext->b)) != NULL)) {

      if (pp->deviceid != lastdevid) {
	// check it's open
	if (fd > 0) {
	  close(fd);
	}
	
	char *fn = threadContext->b->j->devices[pp->deviceid];
	//	fprintf(stderr,"*info* %d / id %zd -> opening %s\n", pp->deviceid, threadContext->id, fn);
	fd = open(fn, O_RDONLY | threadContext->b->o_direct);
	if (fd < 0) {
	  fprintf(stderr,"*info* if the device doesn't support O_DIRECT you may need the '-o' option to turn it off\n");
	  perror(fn);
	}
	lastdevid = pp->deviceid;
      }

      if (pp->action == 'W' && pp->finishTime > 0) {
	// make sure correct comparison
	if (pp->seed != lastseed) {
	  generateRandomBuffer(randombuf, threadContext->b->maxbs, pp->seed);
	  lastseed = pp->seed;
	}
	
	size_t diff = 0;
	memcpy(randombuf, pp, sizeof(size_t));
	int ret = verifyPosition(fd, pp, randombuf, buf, &diff, lastseed, 0, threadContext->b->onlycheckfirst);
	if (ret == 0) {
	  if (verbose) fprintf(stdout, "verify %lf [%zd] %d %zd, num %zd\n", timeAsDouble(), threadContext->id, pp->deviceid, pp->pos, threadContext->b->numPositions);
	  threadContext->correct++;
	} else {
	  fprintf(stdout, "error %lf [%zd] %d %zd, num %zd\n", timeAsDouble(), threadContext->id, pp->deviceid, pp->pos, threadContext->b->numPositions);
	  fprintf(stderr,"*error* incorrect block at position %zd\n", pp->pos);
	  threadContext->wrong++;
	  if (threadContext->wrong > 2) {
	    exit(1);
	  }
	}
	//	fprintf(stderr,"ret = %d\n", ret);
	threadContext->processed++;
	threadContext->bytesProcessed += pp->len;
      }
      
      //usleep(1);
      //      pthread_yield();
    } else {
      usleep(100);
    }
  }
  if (fd > 0) close(fd);
  free(buf);
  free(randombuf);
  //  fprintf(stderr,"*%zd* is finished %zd\n", threadContext->id, threadContext->pleaseStop);
  return NULL;
}

void setupVerificationThreads(blockVerifyType *b) {
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
    threadContext[i].processed = 0;
    threadContext[i].correct = 0;
    threadContext[i].wrong = 0;
    threadContext[i].b = b;

    if (pthread_create(&(pt[i]), NULL, runVerifyThread, &(threadContext[i]))) {
      fprintf(stderr,"*error* can't create a thread\n");
      exit(-1);
    }
  }
  b->isSetup = 1;

  b->nextpos = 0;
  b->numPositions = 0;
  b->maxAllocPositions = 0;
  b->inputComplete = 0;
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
  

blockVerifyType blockVerifySetup(size_t threads, size_t merge, size_t randomize, size_t o_direct, size_t seed, size_t onlycheckfirst) {
  fprintf(stderr,"*info* setup %zd threads, merge %zd, randomize %zd, o_direct %zd, seed %zd, onlycheckfirst %zd\n", threads, merge, randomize, o_direct, seed, onlycheckfirst);

  blockVerifyType b;
  memset(&b, 0, sizeof(blockVerifyType));
  b.seed = seed;
  b.threads = threads;
  b.merge = merge;
  b.randomize = randomize;
  b.o_direct = o_direct;
  b.isSetup = 0;
  b.inQueue = 0;
  b.p = NULL;
  b.minbs = 9e9;
  b.onlycheckfirst = onlycheckfirst;
  b.start = timeAsDouble();

  return b;
}



void blockVerifyStop(blockVerifyType *b) {
  for (size_t i = 1; i <= b->threads; i++) {
    threadContext[i].pleaseStop = 1;
  }
}

void blockVerifyFinish(blockVerifyType *b) {
  b->finish = timeAsDouble();
  size_t bbb = 0, ccc = 0, www = 0;
  for (size_t i = 1; i <= b->threads; i++) {
    pthread_join(pt[i], NULL);
    if (verbose) fprintf(stderr,"*info* [%zd] correct %zd, wrong %zd, total %zd, bytes MB %.1lf\n", i, threadContext[i].correct, threadContext[i].wrong, threadContext[i].processed, TOMB(threadContext[i].bytesProcessed));
    ccc += threadContext[i].correct;
    www += threadContext[i].wrong;
    bbb += threadContext[i].bytesProcessed;
  }
  fprintf(stderr,"*info* %zd correct, %zd wrong, %zd total\n", ccc, www, ccc+www);
  fprintf(stderr,"*info* %.1lf MB processed in %.2lf s is %.2lf MB/s\n", TOMB(bbb), b->finish - b->start, TOMB(bbb) / (b->finish - b->start));
  pthread_join(pt[0], NULL);
  free(pt); pt = NULL;
  free(threadContext); threadContext = NULL;
  pthread_mutex_destroy(&mutex);
  free(b->p); b->p = NULL;
}

void usage(size_t threads) {
  fprintf(stdout,"Usage:\n   ./spitchecker2 [ options] filename\n");
  fprintf(stdout,"\nOptions:\n");
  fprintf(stdout,"   -D    Turn off O_DIRECT\n");
  fprintf(stdout,"   -4    A synonym for -f 1\n");
  fprintf(stdout,"   -f k  Only check the first k KiB\n");
  fprintf(stderr,"   -t n  Specify the number of verification threads to run in parallel (%zd)\n", threads);
  fprintf(stdout,"\nMerge:\n");
  fprintf(stdout,"   -m    Read all positions then merge (default)\n");
  fprintf(stdout,"   -s    Stream positions, don't merge/resolve time clobbering\n");
  fprintf(stdout,"\nOrder:\n");
  fprintf(stdout,"   -p    Sort actions by position per disk (default)\n");
  fprintf(stdout,"   -r    Randomize positions\n");
}

int handle_args(int argc, char *argv[], size_t *threads, size_t *merge, size_t *randomize, size_t *o_direct, size_t *seed, size_t *onlycheckfirst) {
  int opt;
  optind = 0;
  // merge or omerge (stream)
  // random, or sort positions
  // o_direct

  *merge = 1;
  *o_direct = O_DIRECT;
  *randomize = 0; // sorted
  
  const char *getoptstring = "smt:orpR:hVf:4D";
  while ((opt = getopt(argc, argv, getoptstring )) != -1) {
    switch (opt) {
    case 's':
      *merge = 0; // stream;
      break;
    case '4':
      *onlycheckfirst = 4096;
      fprintf(stderr,"*warning* only checking the first %zd bytes\n", *onlycheckfirst);
      break;
    case 'f':
      *onlycheckfirst = 4096 * atoi(optarg);
      fprintf(stderr,"*warning* only checking the first %zd bytes\n", *onlycheckfirst);
      break;
    case 'm':
      *merge = 1; // merge
      break;
    case 'D':
    case 'o':
      *o_direct = 0;
      break;
    case 'r':
      *randomize = 1;
      break;
    case 'p':
      *randomize = 0;
      break;
    case 't':
      *threads = atoi(optarg);
      if (*threads < 1) *threads = 0;
      break;
    case 'R':
      *seed = atoi(optarg);
      break;
    case 'h':
      usage(*threads);
      exit(1);
      break;
    case 'V':
      verbose++;
      break;
    default:
      abort();
    }
  }
  return optind;
}
    
      
      


  

int main(int argc, char *argv[]) {

  keepRunning = 1; verbose = 0;
  size_t threads=256, merge=1, randomize=0, o_direct=0, seed = 42, onlycheckfirst = 0;

  if (argc < 2) {
    usage(threads);
    exit(1);
  }
  
  int ind = handle_args(argc, argv, &threads, &merge, &randomize, &o_direct, &seed, &onlycheckfirst);
  
  blockVerifyType b = blockVerifySetup(threads, merge, randomize, o_direct, seed, onlycheckfirst);
  setupVerificationThreads(&b); // start running, initially no data, so they sleep

  positionContainer pc; positionContainerInit(&pc, 0); // setup position container
  jobType j; jobInit(&j);

  b.j = &j; // make sure you can track j down

  if (merge == 0) {
    b.inputComplete = 1; // start straight away
  }
  // add all the position data
  size_t added = 0;
  size_t lineswitherrors = 0;
  for (int i = ind; i < argc; i++) {
    if (strcmp(argv[i], "-") == 0) {
      while (1) {
	added = positionContainerAddLines(&pc, &j, stdin, 1, &lineswitherrors); // add one line at a time
	addPosition(&b, &pc.positions[pc.sz-1]);
	if (added == 0) break;
      }
    } else {
      if (merge == 0) {
	fprintf(stderr,"*error* you specified a file in stream mode\n");
	exit(1);
      }
      //      fprintf(stderr,"*info* adding %s\n", argv[i]);
      added = positionContainerAddLinesFilename(&pc, &j, argv[i], &lineswitherrors);
      //      fprintf(stderr,"... added %zd\n", added);
    }
  }
  if (merge == 1) {
    // merge, then complete
    positionContainerCollapse(&pc);
    fprintf(stderr,"*info* input complete\n");
    if (randomize == 1) {
      positionsRandomize(pc.positions, pc.sz, seed);
    } else {
      positionsSortPositions(pc.positions, pc.sz);
    }
    positionContainerDump(&pc, 10);

    for (size_t i = 0; i < pc.sz; i++) {
      if (pc.positions[i].action == 'W') {
	addPosition(&b, &pc.positions[i]);
      }
    }

    b.inputComplete = 1;
  }

  // convert from position
  //  setupInputThread(&b); // start reading from stdin on another thread

  while (b.nextpos < b.numPositions) {
    usleep(1000); // check every 1ms that we've finished. How do we wait until the last verify?
  }
  blockVerifyStop(&b); // get the threads to return

  positionContainerFree(&pc); // input data

  jobFree(&j);
  b.j = NULL;

  blockVerifyFinish(&b); // join and continue


  exit(0);
}
