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
#include "devices.h"
#include <math.h>
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

void jobMultiply(jobType *job, const size_t extrajobs, deviceDetails *deviceList, size_t deviceCount) {
  const int origcount = job->count;
  for (size_t i = 0; i < origcount; i++) {
    for (size_t n = 0; n < extrajobs; n++) {
      if (deviceCount == 0) {
	jobAddBoth(job, job->devices[i], job->strings[i]);
      } else {
	for (size_t d = 1; d < deviceCount; d++) {
	  jobAddBoth(job, deviceList[d].devicename, job->strings[i]);
	}
      }
    }
  }
}
  

void jobDump(jobType *job) {
  fprintf(stderr,"*info* jobDump: %zd\n", jobCount(job));
  for (size_t i = 0; i < job->count; i++) {
    fprintf(stderr,"*  info* job %zd, device %s, string %s\n", i, job->devices[i], job->strings[i]);
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
  size_t finishtime;
  size_t waitfor;
  size_t prewait;
  char *jobstring;
  char *jobdevice;
  size_t blockSize;
  size_t highBlockSize;
  size_t queueDepth;
  size_t flushEvery;
  float rw;
  int rerandomize;
  unsigned short seed;
  char *randomBuffer;
  size_t numThreads;
  positionContainer **allPC;
  size_t anywrites;
  size_t UUID;
  char *benchmarkName;
  int oneShot;
  size_t seqFiles;
  size_t runTime;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  if (verbose >= 2) {
    fprintf(stderr,"*info* thread[%zd] job is '%s'\n", threadContext->id, threadContext->jobstring);
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

  fprintf(stderr,"*info* [t%zd] '%s', s%zd, pos=%zd (LBA %.1lf%%, %.1lf GB), rerand=%d, qd=%zd, R/w=%.2g, F=%zd, k=[%zd,%zd], seed %u, oneShot %d, B%zd W%zd T%zd t%zd\n", threadContext->id, threadContext->jobstring, threadContext->seqFiles, threadContext->pos.sz, threadContext->pos.LBAcovered, TOGB(threadContext->bdSize * threadContext->pos.LBAcovered/100.0), threadContext->rerandomize, threadContext->queueDepth, threadContext->rw, threadContext->flushEvery, threadContext->blockSize, threadContext->highBlockSize, threadContext->seed, threadContext->oneShot, threadContext->prewait, threadContext->waitfor, threadContext->runTime, threadContext->finishtime);


  // do the mahi
  double starttime = timedouble();
  sleep(threadContext->prewait);
  while (keepRunning && (timedouble() < starttime + threadContext->finishtime)) {
    sleep(threadContext->waitfor);
    aioMultiplePositions(&threadContext->pos, threadContext->pos.sz, timedouble() + threadContext->runTime, threadContext->queueDepth, -1 /* verbose */, 0, NULL, &benchl, threadContext->randomBuffer, threadContext->highBlockSize, MIN(4096,threadContext->blockSize), &ios, &shouldReadBytes, &shouldWriteBytes, threadContext->oneShot || threadContext->rerandomize, 1, fd, threadContext->flushEvery);
    if (!keepRunning) {fprintf(stderr,"*info* finished...\n");}
    if (threadContext->oneShot) {
      break;
    }
    if (threadContext->rerandomize) {
      positionRandomize(threadContext->pos.positions, threadContext->pos.sz);
      if (verbose >= 1) {
	fprintf(stderr,"*info* randomizing\n");
	fprintf(stderr,"*info* first pos %zd\n", threadContext->pos.positions[0].pos);
      }
    }
  }

  if (verbose) fprintf(stderr,"*info [thread %zd] finished '%s'\n", threadContext->id, threadContext->jobstring);
  threadContext->pos.elapsedTime = timedouble() - starttime;

  fdatasync(fd); // make sure all the data is on disk before we axe off the ioc
  
  close(fd);

  if (threadContext->benchmarkName) {
    char s[1024];
    sprintf(s, "%s-%03zd", threadContext->benchmarkName, threadContext->id);
    
    logSpeedDump(&benchl, s, 0, "", threadContext->bdSize, threadContext->bdSize, 0, 0, 0, 0, 0, "");
    logSpeedFree(&benchl);
  }

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

  double starttime = timedouble();
  
  while (keepRunning && (thistime = timedouble())) {
    usleep(10000);
    //    usleep(500000);

    if (thistime - start >= (i * TIMEPERLINE) && (thistime < starttime + threadContext->finishtime+0.1)) {
      
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
      commaPrint0dp(stderr, TOMB(trb - last_trb));
      fprintf(stderr," MB/s (");
      commaPrint0dp(stderr, tri - last_tri);
      fprintf(stderr," IOPS / %zd), write ", (tri - last_tri == 0) ? 0 : (trb - last_trb) / (tri - last_tri));
      commaPrint0dp(stderr, TOMB(twb - last_twb));
      fprintf(stderr," MB/s (");
      commaPrint0dp(stderr, twi - last_twi);
      //      fprintf(stderr," IOPS / %zd), util %.0lf %%\n", (twi - last_twi == 0) ? 0 : (twb - last_twb) / (twi - last_twi), util);
      fprintf(stderr," IOPS / %zd)\n", (twi - last_twi == 0) ? 0 : (twb - last_twb) / (twi - last_twi));

      last_trb = trb;
      last_tri = tri;
      last_twb = twb;
      last_twi = twi;
      
      i++;
    }

    if (thistime > starttime + threadContext->finishtime + 30) {
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
		   const size_t timetorun, const size_t dumpPos, char *benchmarkName) {
  pthread_t *pt;
  CALLOC(pt, num+1, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num+1, sizeof(threadInfoType));

  keepRunning = 1;
  
  unsigned short seed = (unsigned short)timedouble();

  positionContainer **allThreadsPC;
  CALLOC(allThreadsPC, num, sizeof(positionContainer*));

  const double thetime = timedouble();
  const size_t UUID = thetime * 10;

  
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

    charBS = strchr(job->strings[i], 'M');
    if (charBS && *(charBS+1)) {

      char *endp = NULL;
      bs = 1024 * 1024 * strtod(charBS+1, &endp);
      if (bs < 512) bs = 512;
      highbs = bs;
      if (*endp == '-') {
	int nextv = atoi(endp+1);
	if (nextv > 0) {
	  highbs = 1024 * 1024 * nextv;
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
    threadContext[i].benchmarkName = benchmarkName;
    threadContext[i].UUID = UUID;
    positionContainerInit(&threadContext[i].pos, threadContext[i].UUID);
    threadContext[i].jobstring = job->strings[i];
    threadContext[i].jobdevice = job->devices[i];
    threadContext[i].waitfor = 0;
    threadContext[i].prewait = 0;
    threadContext[i].runTime = 0;
    threadContext[i].finishtime = 0;
    threadContext[i].blockSize = bs;
    threadContext[i].highBlockSize = highbs;
    threadContext[i].rerandomize = 0;

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

    char *multLimit = strchr(job->strings[i], 'x');
    size_t multiply = (size_t)1;
    if (seqFiles == 0) multiply = 2;

    if (multLimit && *(multLimit+1)) {
      char *endp = NULL;
      multiply = (size_t) (strtod(multLimit+1, &endp));
    }


    {
      char *iR = strchr(job->strings[i], 'n');
      if (iR) {// && *(iR+1)) {
	threadContext[i].rerandomize = 1;
      }
    }



    int iTime = timetorun;
    threadContext[i].finishtime = iTime;

    iTime = timetorun;
    {
      char *iTO = strchr(job->strings[i], 'T');
      if (iTO) {
	iTime = atoi(iTO+1);
      }
    }
    threadContext[i].runTime = iTime;


    int oneShot = 0;
    {
      char *oOS = strchr(job->strings[i], 'O');
      if (oOS) {
	oneShot = 1;
	threadContext[i].runTime = -1;
      }
    }
    threadContext[i].oneShot = oneShot;



    size_t metaData = 0;
    {
      // metaData mode is random, verify all writes and flushing
      char *iR = strchr(job->strings[i], 'm');
      if (iR) {
	metaData = 1;
	seqFiles = 0;
	threadContext[i].flushEvery = 1;
      }
    }

    int qDepth = 512; // 512 is the default
    {
      char *qdd = strchr(job->strings[i], 'q');
      if (qdd && *(qdd+1)) {
	qDepth = atoi(qdd+1);
      }
    }
    if (qDepth < 1) qDepth = 1;

    char *pChar = strchr(job->strings[i], 'P');
    {
      if (pChar && *(pChar+1)) {
	size_t newmp = atoi(pChar + 1);
	if (newmp < mp) {
	  mp = newmp;
	}
	if (mp < 1) mp = 1;
	if (mp * multiply <= qDepth) {
	  qDepth = mp;
	}
      }
    }

    threadContext[i].queueDepth = qDepth;


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

    Wchar = strchr(job->strings[i], 'B');
    if (Wchar && *(Wchar+1)) {
      size_t waitfor = atoi(Wchar + 1);
      if (waitfor > timetorun) {
	waitfor = timetorun;
	fprintf(stderr,"*warning* waiting decreased to %zd\n", waitfor);
      }
      threadContext[i].prewait = waitfor;
    }

    threadContext[i].pos.bdSize = threadContext[i].bdSize;

    
    /*if (iRandom == 0) */{ // if iRandom set, then don't setup positions here, do it in the runThread. e.g. -c n
      positionContainerSetup(&threadContext[i].pos, mp, job->devices[i], job->strings[i]);

      // allocate the position array space
      //positionContainerSetup(&threadContext[i].pos, mp, job->devices[i], job->strings[i]);
      // create the positions and the r/w status
      threadContext[i].seqFiles = seqFiles;
      size_t anywrites = setupPositions(threadContext[i].pos.positions, &threadContext[i].pos.sz, threadContext[i].seqFiles, rw, threadContext[i].blockSize, threadContext[i].highBlockSize, MIN(4096,threadContext[i].blockSize), startingBlock, threadContext[i].bdSize, threadContext[i].seed);
      threadContext[i].pos = positionContainerMultiply(&threadContext[i].pos, multiply);

      if (verbose >= 2) {
	checkPositionArray(threadContext[i].pos.positions, threadContext[i].pos.sz, threadContext[i].bdSize, !metaData);
      }

      
      if (threadContext[i].seqFiles == 0) positionRandomize(threadContext[i].pos.positions, threadContext[i].pos.sz);
      threadContext[i].anywrites = anywrites;
      calcLBA(&threadContext[i].pos); // calc LBA coverage

      
      if (metaData) {
	positionContainerAddMetadataChecks(&threadContext[i].pos);
      }

      if (dumpPos /* && !iRandom*/) {
	dumpPositions(threadContext[i].pos.positions, threadContext[i].pos.string, threadContext[i].pos.sz, dumpPos);
      }
    }

    /*    if (mp <= threadContext[i].queueDepth) {
      threadContext[i].queueDepth = mp;
      }*/


    // add threadContext[i].pos a pointer alias to a pool for the timer
    allThreadsPC[i] = &threadContext[i].pos;
    threadContext[i].numThreads = num;
    threadContext[i].allPC = allThreadsPC;
  }

  // set the starting time
  for (size_t i = 0; i < num; i++) { 
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
    positionLatencyStats(&threadContext[i].pos, i);
  }

  positionContainer *origpc;
  CALLOC(origpc, num, sizeof(positionContainer));
  for (size_t i = 0; i < num; i++) {
    origpc[i] = threadContext[i].pos;
  }
  
  positionContainer mergedpc = positionContainerMerge(origpc, num);

  char s[1000];
  sprintf(s, "spit-positions.txt");
  fprintf(stderr, "*info* writing positions to '%s'\n", s); 
  positionContainerSave(&mergedpc, s, mergedpc.bdSize, 0);
  
  // free
  for (size_t i = 0; i < num; i++) {
    positionContainerFree(&threadContext[i].pos);
    free(threadContext[i].randomBuffer);
  }

  positionContainerFree(&mergedpc);
  free(origpc);
  
  free(allThreadsPC);
  free(threadContext);
  free(pt);
}


void jobAddDeviceToAll(jobType *job, const char *device) {
  for (size_t i = 0; i < job->count; i++) {
    job->devices[i] = strdup(device);
  }
}

size_t jobCount(jobType *job) {
  return job->count;
}
