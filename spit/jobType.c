#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>

#include <assert.h>
#include <pthread.h>

#include "jobType.h"
#include "positions.h"
#include "utils.h"

#include "aioRequests.h"
#include "diskStats.h"

extern volatile int keepRunning;
extern int verbose;

void jobInit(jobType *job) {
  job->count = 0;
  job->strings = NULL;
  job->devices = NULL;
}

void jobAddBoth(jobType *job, char *device, char *jobstring) {
  job->strings = realloc(job->strings, (job->count+1) * sizeof(char*));
  job->devices = realloc(job->devices, (job->count+1) * sizeof(char*));
  job->strings[job->count] = strdup(jobstring);
  job->devices[job->count] = strdup(device);
  job->count++;
}

void jobAdd(jobType *job, const char *jobstring) {
  job->strings = realloc(job->strings, (job->count+1) * sizeof(char*));
  job->devices = realloc(job->devices, (job->count+1) * sizeof(char*));
  job->strings[job->count] = strdup(jobstring);
  job->devices[job->count] = NULL;
  job->count++;
}

void jobMultiply(jobType *job, const size_t extrajobs) {
  const int origcount = job->count;
  for (size_t i = 0; i < origcount; i++) {
    for (size_t n = 0; n < extrajobs; n++) {
      jobAddBoth(job, job->devices[i], job->strings[i]);
    }
  }
}
  

void jobDump(jobType *job) {
  for (size_t i = 0; i < job->count; i++) {
    fprintf(stderr,"*info* job %zd, device %s, string %s\n", i, job->devices[i], job->strings[i]);
  }
}

void jobDumpAll(jobType *job) {
  for (size_t i = 0; i < job->count; i++) {
    fprintf(stderr,"*info* job %zd, string %s\n", i, job->strings[i]);
  }
}

void jobFree(jobType *job) {
  for (size_t i = 0; i < job->count; i++) {
    if (job->strings[i]) {free(job->strings[i]); job->strings[i] = NULL;}
    if (job->devices[i]) {free(job->devices[i]); job->devices[i] = NULL;}
  }
  free(job->strings); job->strings = NULL;
  free(job->devices); job->devices = NULL;
  jobInit(job);
}

typedef struct {
  size_t id;
  positionContainer pos;
  size_t bdSize;
  double finishtime;
  size_t waitfor;
  char *jobstring;
  char *jobdevice;
  size_t blockSize;
  size_t highBlockSize;
  size_t queueDepth;
  size_t flushEvery;
  float rw;
  size_t random;
  unsigned short seed;
  char *randomBuffer;
  size_t numThreads;
  positionContainer **allPC;
  size_t anywrites;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  if (verbose >= 2) {
    fprintf(stderr,"*info* thread[%zd] job is '%s'\n", threadContext->id, threadContext->pos.string);
  }

  logSpeedType benchl;
  logSpeedInit(&benchl);



  size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
  int fd,  direct = O_DIRECT;
  if (strchr(threadContext->jobstring, 'D')) {
    fprintf(stderr,"*info* thread[%zd] turning off O_DIRECT\n", threadContext->id);
    direct = 0; // don't use O_DIRECT if the user specifes 'D'
  }

  if (threadContext->anywrites) {
    fd = open(threadContext->jobdevice, O_RDWR | direct);
    if (verbose >= 2) fprintf(stderr,"*info* open with O_RDWR\n");
  } else {
    fd = open(threadContext->jobdevice, O_RDONLY | direct);
    if (verbose >= 2) fprintf(stderr,"*info* open with O_RDONLY\n");
  }
    
  if (fd < 0) {
    fprintf(stderr,"problem!!\n");
    perror(threadContext->jobdevice); return 0;
  }

  
  if (threadContext->waitfor) {
    if (verbose >= 2) {
      fprintf(stderr,"*info* thread[%zd] waiting for %zd seconds\n", threadContext->id, threadContext->waitfor);
    }
    sleep(threadContext->waitfor);
  }


  fprintf(stderr,"*info* [t%zd] '%s' pos=%zd, |%zd|, qd=%zd, R/w=%.2g, F=%zd, k=[%zd,%zd], seed %u\n", threadContext->id, threadContext->jobstring, threadContext->pos.sz, threadContext->random, threadContext->queueDepth, threadContext->rw, threadContext->flushEvery, threadContext->blockSize, threadContext->highBlockSize, threadContext->seed);

  double start = timedouble();
  if (threadContext->random > 0) {
    size_t s = threadContext->id + threadContext->pos.sz;
    positionContainer pc;
    positionContainerInit(&pc);
    
    positionType *p = createPositions(threadContext->random);
    pc.positions = p;
    while (keepRunning && timedouble() < threadContext->finishtime) {
      size_t anywrites = setupRandomPositions(p, threadContext->random, threadContext->rw, threadContext->blockSize, threadContext->highBlockSize, threadContext->blockSize, threadContext->bdSize, s++);
      threadContext->anywrites = anywrites;
      
      if (verbose >= 2) {
	fprintf(stderr,"*info* generating random %zd\n", threadContext->random);
	dumpPositions(p, "random", threadContext->random, 10);
      }

      aioMultiplePositions(&pc, threadContext->random, threadContext->finishtime, threadContext->queueDepth, -1 /*verbose*/, 0, NULL, &benchl, threadContext->randomBuffer, threadContext->highBlockSize, MIN(4096, threadContext->blockSize), &ios, &shouldReadBytes, &shouldWriteBytes, 1 /* one shot*/, 1, fd, threadContext->flushEvery);
    }


    freePositions(p);
  } else {
    aioMultiplePositions(&threadContext->pos, threadContext->pos.sz, threadContext->finishtime, threadContext->queueDepth, -1 /* verbose */, 0, NULL, &benchl, threadContext->randomBuffer, threadContext->highBlockSize, MIN(4096,threadContext->blockSize), &ios, &shouldReadBytes, &shouldWriteBytes, 0, 1, fd, threadContext->flushEvery);
  }
  fprintf(stderr,"*info [thread %zd] finished '%s'\n", threadContext->id, threadContext->jobstring);
  threadContext->pos.elapsedTime = timedouble() - start;

  close(fd);

  logSpeedFree(&benchl);
  close(fd);

  return NULL;
}



#define TIMEPERLINE 1

static void *runThreadTimer(void *arg) {
  const threadInfoType *threadContext = (threadInfoType*)arg;

  size_t i = 1;
  //  diskStatType d;
  //  diskStatSetup(&d);
  int fd = open(threadContext->jobdevice, O_RDONLY);
  if (fd < 0) {
    perror("diskstats"); return NULL;
  }
  //  diskStatAddDrive(&d, fd);
  close(fd);
  
  //  diskStatStart(&d);

  const double start = timedouble();
  double thistime = 0;
  size_t last_trb = 0, last_twb = 0, last_tri = 0, last_twi = 0;
  size_t trb = 0, twb = 0, tri = 0, twi = 0;

  while (keepRunning && (thistime = timedouble())) {
    usleep(100000);
    //    usleep(500000);

    if (thistime - start >= (i * TIMEPERLINE) && (thistime <= threadContext->finishtime)) {
      
      //      diskStatFinish(&d);
      trb = 0;
      twb = 0;
      tri = 0;
      twi = 0;
      //      double util = 0;
      //diskStatSummary(&d, &trb, &twb, &tri, &twi, &util, 0, 0, 0, thistime - last);

      for (size_t j = 0; j < threadContext->numThreads;j++) {
	trb += threadContext->allPC[j]->readBytes;
	tri += threadContext->allPC[j]->readIOs;
	twb += threadContext->allPC[j]->writtenBytes;
	twi += threadContext->allPC[j]->writtenIOs;
      }
      
      const double elapsed = thistime - start;

      

      fprintf(stderr,"[%2.0lf / %zd] read ", elapsed, threadContext->numThreads);
      commaPrint0dp(stderr, TOMiB(trb - last_trb));
      fprintf(stderr," MiB/s (");
      commaPrint0dp(stderr, tri - last_tri);
      fprintf(stderr," IOPS / %zd), write ", (tri - last_tri == 0) ? 0 : (trb - last_trb) / (tri - last_tri));
      commaPrint0dp(stderr, TOMiB(twb - last_twb));
      fprintf(stderr," MiB/s (");
      commaPrint0dp(stderr, twi - last_twi);
      //      fprintf(stderr," IOPS / %zd), util %.0lf %%\n", (twi - last_twi == 0) ? 0 : (twb - last_twb) / (twi - last_twi), util);
      fprintf(stderr," IOPS / %zd)\n", (twi - last_twi == 0) ? 0 : (twb - last_twb) / (twi - last_twi));

      last_trb = trb;
      last_tri = tri;
      last_twb = twb;
      last_twi = twi;
      

      //    fprintf(stderr,"[%2.0lf] read %.0lf MiB/s (%zd IOPS), write %.0lf MiB/s (%zd IOPS), util %.0lf %%\n", elapsed, TOMiB(trb), tri, TOMiB(twb), twi, util);
      
      //      last = thistime;
      
      i++;
      //      diskStatStart(&d);
    }

    if (thistime > threadContext->finishtime + 10) {
      fprintf(stderr,"*error* still running! watchdog exit\n");
      exit(-1);
    }
  }
  //  diskStatFree(&d);
  //  fprintf(stderr,"finished thread timer\n");
  keepRunning = 0;
  return NULL;
}



void jobRunThreads(jobType *job, const int num, const size_t maxSizeInBytes,
		   const size_t timetorun, const size_t dumpPositionsN) {
  pthread_t *pt;
  CALLOC(pt, num+1, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num+1, sizeof(threadInfoType));

  keepRunning = 1;
  
  unsigned short seed = (unsigned short)timedouble();

  size_t newmp = 0;

  positionContainer **allThreadsPC;
  CALLOC(allThreadsPC, num, sizeof(positionContainer*));
  
  for (size_t i = 0; i < num; i++) {

    int seqFiles = 1;
    int bs = 4096, highbs = 4096;

    if (verbose) {
      fprintf(stderr,"*info* setting up thread %zd\n", i);
    }
    char *charBS = strchr(job->strings[i], 'k');
    if (charBS && *(charBS+1)) {

      char *endp = NULL;
      bs = 1024 * strtod(charBS+1, &endp);
      if (bs < 512) bs = 512;
      highbs = bs;
      if (*endp == '-') {
	int nextv = atoi(endp+1);
	if (nextv > 0) {
	  highbs = 1024 * nextv;
	}
      }
    }
    if (highbs < bs) {
      highbs = bs;
    }
    

    char *charLimit = strchr(job->strings[i], 'L');
    size_t limit = (size_t)-1;

    if (charLimit && *(charLimit+1)) {
      char *endp = NULL;
      limit = (size_t) (strtod(charLimit+1, &endp) * 1024 * 1024 * 1024);
      if (limit == 0) limit = (size_t)-1;
    }
    

    //    size_t fs = fileSizeFromName(job->devices[i]);
    threadContext[i].bdSize = maxSizeInBytes;
    const size_t avgBS = (bs + highbs) / 2;
    size_t mp = (size_t) (threadContext[i].bdSize / avgBS);
    if (verbose) {
      fprintf(stderr,"*info* file size %.3lf GiB avg size of %zd, maximum ", TOGiB(threadContext[i].bdSize), avgBS);
      commaPrint0dp(stderr, mp);
      fprintf(stderr," positions\n");
    }

    //    size_t fitinram = totalRAM() / 4 / num / sizeof(positionType);
    size_t useRAM = 2L*1024*1024*1024;
    size_t fitinram = useRAM / num / sizeof(positionType);
    if (verbose || (fitinram < mp)) {
      fprintf(stderr,"*info* using %.3lf GiB RAM for positions, we can store ", TOGiB(useRAM));
      commaPrint0dp(stderr, fitinram);
      fprintf(stderr," positions\n");
    }
    
    size_t countintime = mp;
    if ((long)timetorun > 0) { // only limit based on time if the time is positive
      
#define ESTIMATEIOPS 500000
      
      countintime = timetorun * ESTIMATEIOPS;
      if (verbose || (countintime < mp)) {
	fprintf(stderr,"*info* in %zd seconds, at %d a second, would have at most ", timetorun, ESTIMATEIOPS);
	commaPrint0dp(stderr, countintime);
	fprintf(stderr," positions\n");
      }
    }
    size_t sizeLimitCount = (size_t)-1;
    if (limit != (size_t)-1) {
      sizeLimitCount = limit / avgBS;
      fprintf(stderr,"*info* to limit sum of lengths to %.1lf GiB, with avg size of %zd, requires %zd positions\n", TOGiB(limit), avgBS, sizeLimitCount);
    }

    size_t mp2 = MIN(sizeLimitCount, MIN(countintime, MIN(mp, fitinram)));
    if (mp2 != mp) {
      mp = mp2;
      fprintf(stderr,"*info* positions limited to ");
      commaPrint0dp(stderr, mp);
      fprintf(stderr,"\n");
    }
      
    threadContext[i].id = i;
    threadContext[i].jobstring = job->strings[i];
    threadContext[i].jobdevice = job->devices[i];
    positionContainerInit(&threadContext[i].pos);
    threadContext[i].waitfor = 0;
    threadContext[i].blockSize = bs;
    threadContext[i].highBlockSize = highbs;
    threadContext[i].random = 0;

    // do this here to allow repeatable random numbers
    int rcount = 0, wcount = 0, rwtotal = 0;
    float rw = 0;
    for (size_t k = 0; k < strlen(job->strings[i]); k++) {
      if (job->strings[i][k] == 'r') {
	rcount++;
	rwtotal++;
      } else if (job->strings[i][k] == 'w') {
	wcount++;
	rwtotal++;
      }
    }
    if (rwtotal == 0) {
      rw = 0.5; // default to 50/50 mix read/write
    } else {
      rw = rcount * 1.0 / rwtotal;
    }
    threadContext[i].rw = rw;

    int flushEvery = 0;
    for (size_t k = 0; k < strlen(job->strings[i]); k++) {
      if (job->strings[i][k] == 'F') {
	if (flushEvery == 0) {
	  flushEvery = 1;
	} else {
	  flushEvery *= 10;
	}
      }
    }
    threadContext[i].flushEvery = flushEvery;
    
    {
      char *sf = strchr(job->strings[i], 's');
      if (sf && *(sf+1)) {
	seqFiles = atoi(sf+1);
      }
    }

    int iRandom = 0;
    {
      char *iR = strchr(job->strings[i], 'n');
      if (iR) {// && *(iR+1)) {
	iRandom = 1000000;
	//	mp = iRandom;
	//	iRandom = atoi(iR+1);
	//	fprintf(stderr,"ir %d mp %zd\n", iRandom, mp);
      }
    }
    threadContext[i].random = iRandom;



    size_t repeat = 0;
    {
      char *iR = strchr(job->strings[i], 'm');
      if (iR) {
	repeat = 1;
	seqFiles = 0;
	flushEvery = 1;
	threadContext[i].flushEvery = flushEvery;
      }
    }

    int qDepth = 256;
    {
      char *qdd = strchr(job->strings[i], 'q');
      if (qdd && *(qdd+1)) {
	qDepth = atoi(qdd+1);
      }
    }
    if (qDepth < 1) qDepth = 1;
    threadContext[i].queueDepth = qDepth;
    
    char *pChar = strchr(job->strings[i], 'P');
    {
      if (pChar && *(pChar+1)) {
	newmp = atoi(pChar + 1);
	if (newmp >= mp) {
	  newmp = 0; // ignored
	}
      }
    }

    long startingBlock = -99999;

    if (strchr(job->strings[i], 'z')) {
      startingBlock = 0;
    }

    {
      char *RChar = strchr(job->strings[i], 'R');
      if (RChar && *(RChar+1)) {
	seed = atoi(RChar + 1); // if specified
      } else {
	if (i > 0) seed++;
      }
    }
    threadContext[i].seed = seed;

    char *Wchar = strchr(job->strings[i], 'W');
    if (Wchar && *(Wchar+1)) {
      size_t waitfor = atoi(Wchar + 1);
      if (waitfor > timetorun) {
	waitfor = timetorun;
	fprintf(stderr,"*warning* waiting decreased to %zd\n", waitfor);
      }
      
      threadContext[i].waitfor = waitfor;
    }

    /*    if (newmp && (newmp < mp)) {
      fprintf(stderr,"*info* positions overwritten to %zd using 'P'\n", newmp);
      mp = newmp;
      }*/

    
    if (!iRandom) {
      if (repeat == 0) {
	//	fprintf(stderr,"*info* (1) creating %zd positions...\n", mp); fflush(stderr);
	positionContainerSetup(&threadContext[i].pos, mp, job->devices[i], job->strings[i]);
	size_t anywrites = setupPositions(threadContext[i].pos.positions, &threadContext[i].pos.sz, seqFiles, rw, bs, highbs, bs, startingBlock, threadContext[i].bdSize, threadContext[i].seed);
	// 'P' override
	if (newmp && (newmp < mp)) {
	  threadContext[i].pos.sz = newmp;
	}
	threadContext[i].anywrites = anywrites;

      } else {
	// allocate twice as much
	if (2 * newmp < mp) {
	  mp = 2*newmp;
	}
	fprintf(stderr,"*info* (2) creating %zd positions...\n", mp); 
	positionContainerSetup(&threadContext[i].pos, mp, job->devices[i], job->strings[i]);

	// split into two halves
	size_t sz1 = mp / 2; // setup the first 1/2
	size_t anywrites = setupPositions(threadContext[i].pos.positions, &sz1, seqFiles, rw, bs, highbs, bs, startingBlock, threadContext[i].bdSize, threadContext[i].seed);
	threadContext[i].anywrites = anywrites;
	
	// copy again
	for (size_t j = 0; j < sz1; j++) {
	  positionType *p = &threadContext[i].pos.positions[sz1 + j];
	  
	  *p = threadContext[i].pos.positions[j];
	  if (p->action == 'W') { // it's just a pointer check so check all reads
	    p->action = 'R'; //
	    p->verify = 1;
	  }
	}
	threadContext[i].pos.sz = mp;
	if (mp / 2 < threadContext[i].queueDepth) {
	  threadContext[i].queueDepth = mp / 2;
	}
      }
	
      //      fprintf(stderr,"\n");
      if (verbose) {
	checkPositionArray(threadContext[i].pos.positions, threadContext[i].pos.sz, threadContext[i].bdSize, !repeat);
      }
      if (dumpPositionsN) {
	dumpPositions(threadContext[i].pos.positions, threadContext[i].pos.string, threadContext[i].pos.sz, dumpPositionsN);
      }
    }


    // add threadContext[i].pos a pointer alias to a pool for the timer
    allThreadsPC[i] = &threadContext[i].pos;
    threadContext[i].numThreads = num;
    threadContext[i].allPC = allThreadsPC;
  }

  // set the starting time
  const double currenttime = timedouble();
  const double finishtime = currenttime + timetorun;
  assert (finishtime >= currenttime + timetorun);
  for (size_t i = 0; i < num; i++) {
    threadContext[i].finishtime = finishtime;

    CALLOC(threadContext[i].randomBuffer, threadContext[i].highBlockSize, 1);
    memset(threadContext[i].randomBuffer, 0, threadContext[i].highBlockSize);
    generateRandomBufferCyclic(threadContext[i].randomBuffer, threadContext[i].highBlockSize, threadContext[i].seed, threadContext[i].highBlockSize); 
  }

  
  // use the device and timing info from context[0]
  pthread_create(&(pt[num]), NULL, runThreadTimer, &(threadContext[0]));
  for (size_t i = 0; i < num; i++) {
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }

  // wait for all threads
  for (size_t i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
  }
  keepRunning = 0; // the 
  // now wait for the timer thread (probably don't need this)
  pthread_join(pt[num], NULL);

    
  // print stats 
  for (size_t i = 0; i < num; i++) {
    if (!threadContext[i].random) {
      positionLatencyStats(&threadContext[i].pos, i);
    }
  }

  //        if (logPositions) {
  for (size_t i = 0; i < num; i++) {
    char s[1000];
    sprintf(s, "spit-positions-%03zd.txt", i);
    fprintf(stderr, "*info* writing positions to '%s'\n", s); 
    positionContainerSave(&threadContext[i].pos, s, threadContext[i].bdSize, 0);
  }
  //    } 


  
  // free
  for (size_t i = 0; i < num; i++) {
    positionContainerFree(&threadContext[i].pos);
    free(threadContext[i].randomBuffer);
  }

  free(allThreadsPC);
  free(threadContext);
  free(pt);
}


void jobAddDeviceToAll(jobType *job, const char *device) {
  for (size_t i = 0; i < job->count; i++) {
    job->devices[i] = strdup(device);
  }
}
    
