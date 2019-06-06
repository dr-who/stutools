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
  size_t runXtimes;
  unsigned short seed;
  char *randomBuffer;
  size_t numThreads;
  positionContainer **allPC;
  char *benchmarkName;
  size_t anywrites;
  size_t UUID;
  size_t seqFiles;
  size_t runTime;
  size_t ignoreResults;
  size_t exitIOPS;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  if (verbose >= 2) {
    fprintf(stderr,"*info* thread[%zd] job is '%s'\n", threadContext->id, threadContext->jobstring);
  }

  size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
  int fd,  direct = O_DIRECT;
  if (strchr(threadContext->jobstring, 'D')) {
    fprintf(stderr,"*info* thread[%zd] turning off O_DIRECT\n", threadContext->id);
    direct = 0; // don't use O_DIRECT if the user specifes 'D'
  }

  // check block sizes
  {
    char *suffix = getSuffix(threadContext->jobdevice);
    size_t phybs, logbs;
    getPhyLogSizes(suffix, &phybs, &logbs);
    //    fprintf(stderr,"*info* device %s, physical io size %zd, logical io size %zd\n", threadContext->jobdevice, phybs, logbs);
    free(suffix);

    for (size_t i = 0; i < threadContext->pos.sz; i++) {
      if (threadContext->pos.positions[i].len < logbs) {
	fprintf(stderr,"*error* device '%s' doesn't support size %d [logical bs %zd]\n", threadContext->jobdevice, threadContext->pos.positions[i].len, logbs);
	exit(1);
      }
    }
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

  char *suffix = getSuffix(threadContext->jobdevice);
  if (getWriteCache(suffix) > 0) {
    fprintf(stderr,"*****************\n");
    fprintf(stderr,"* w a r n i n g * device %s is configured with volatile cache\n", threadContext->jobdevice);
    fprintf(stderr,"* The following results are not ENTERPRISE certified. \n");
    fprintf(stderr,"* /sys/block/%s/queue/write_cache should be 'write through'.\n", suffix);
    //    fprintf(stderr,"* Results file has been disabled.\n");
    //    threadContext->ignoreResults = 1;
    fprintf(stderr,"*****************\n");
  }
  if (suffix) free(suffix);

  if (threadContext->finishtime < threadContext->runTime) {
    fprintf(stderr,"*warning* timing %zd > %zd doesn't make sense\n", threadContext->runTime, threadContext->finishtime);
  }
  fprintf(stderr,"*info* [t%zd] '%s', s%zd, pos=%zd (LBA %.1lf%%, %.1lf GB), n=%d, qd=%zd, R/w=%.2g, F=%zd, k=[%zd,%zd], seed/R=%u, B%zd W%zd T%zd t%zd X%zd\n", threadContext->id, threadContext->jobstring, threadContext->seqFiles, threadContext->pos.sz, threadContext->pos.LBAcovered, TOGB(threadContext->bdSize * threadContext->pos.LBAcovered/100.0), threadContext->rerandomize, threadContext->queueDepth, threadContext->rw, threadContext->flushEvery, threadContext->blockSize, threadContext->highBlockSize, threadContext->seed, threadContext->prewait, threadContext->waitfor, threadContext->runTime, threadContext->finishtime, threadContext->runXtimes);


  // do the mahi
  double starttime = timedouble();
  sleep(threadContext->prewait);
  size_t iteratorCount = 0;

  while (keepRunning) {
    if (threadContext->runXtimes && iteratorCount >= threadContext->runXtimes) break;
    if (timedouble() > starttime + threadContext->finishtime) break;
    sleep(threadContext->waitfor);
    aioMultiplePositions(&threadContext->pos, threadContext->pos.sz, timedouble() + threadContext->runTime, threadContext->queueDepth, -1 /* verbose */, 0, NULL, NULL /*&benchl*/, threadContext->randomBuffer, threadContext->highBlockSize, MIN(4096,threadContext->blockSize), &ios, &shouldReadBytes, &shouldWriteBytes, threadContext->runXtimes || threadContext->rerandomize, 1, fd, threadContext->flushEvery);
    if (!keepRunning && threadContext->id == 0) {fprintf(stderr,"*info* finished...\n");}
    if (threadContext->runXtimes == 1) {
      break;
    }
    if (threadContext->rerandomize) {
      positionRandomize(threadContext->pos.positions, threadContext->pos.sz);
      if (verbose >= 1) {
	fprintf(stderr,"*info* randomizing\n");
	fprintf(stderr,"*info* first pos %zd\n", threadContext->pos.positions[0].pos);
      }
    }
    iteratorCount++;      
    if (verbose) fprintf(stderr,"*info* finished pass %zd\n", iteratorCount);
  }

  if (verbose) fprintf(stderr,"*info [thread %zd] finished '%s'\n", threadContext->id, threadContext->jobstring);
  threadContext->pos.elapsedTime = timedouble() - starttime;

  //  fdatasync(fd); // make sure all the data is on disk before we axe off the ioc

  close(fd);

  return NULL;
}



#define TIMEPERLINE 1

static void *runThreadTimer(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;

  size_t i = 1;

  const double start = timedouble();
  double thistime = 0;
  size_t last_trb = 0, last_twb = 0, last_tri = 0, last_twi = 0;
  size_t trb = 0, twb = 0, tri = 0, twi = 0;

  double starttime = timedouble();

  FILE *fp = NULL;
  if (threadContext->benchmarkName) {
    fp = fopen(threadContext->benchmarkName, "wt");
    if (fp) {
      fprintf(stderr,"*info* benchmark file '%s'\n", threadContext->benchmarkName);
    } else {
      fprintf(stderr,"*error* couldn't create benchmark file '%s'\n", threadContext->benchmarkName);
      perror(threadContext->benchmarkName);
    }
  }

  size_t exitcount = 0;
  while (keepRunning && (thistime = timedouble())) {
    usleep(10000);

    if (thistime - start >= (i * TIMEPERLINE) && (thistime < starttime + threadContext->finishtime+0.1)) {
      
      trb = 0;
      twb = 0;
      tri = 0;
      twi = 0;

      size_t devicerb = 0, devicewb = 0;
      //      double util = 0;

      for (size_t j = 0; j < threadContext->numThreads;j++) {
	if (!threadContext->ignoreResults) {
	  trb += threadContext->allPC[j]->readBytes;
	  tri += threadContext->allPC[j]->readIOs;
	  twb += threadContext->allPC[j]->writtenBytes;
	  twi += threadContext->allPC[j]->writtenIOs;
	}
      }
      if (threadContext->pos.diskStats) {
	diskStatFinish(threadContext->pos.diskStats);
	devicerb += diskStatTBRead(threadContext->pos.diskStats);
	devicewb += diskStatTBWrite(threadContext->pos.diskStats);
	diskStatStart(threadContext->pos.diskStats); // reset
      }
      
      const double elapsed = thistime - start;

      size_t readB     = trb - last_trb;
      size_t readIOPS  = tri - last_tri;

      size_t writeB    = twb - last_twb;
      size_t writeIOPS = twi - last_twi;

      fprintf(stderr,"[%2.0lf / %zd] read ", elapsed, threadContext->numThreads);
      commaPrint0dp(stderr, TOMB(readB));
      fprintf(stderr," MB/s (");
      commaPrint0dp(stderr, readIOPS);
      fprintf(stderr," IOPS / %zd), write ", (readIOPS == 0) ? 0 : (readB) / (readIOPS));
      commaPrint0dp(stderr, TOMB(writeB));
      fprintf(stderr," MB/s (");
      commaPrint0dp(stderr, writeIOPS);
      double readamp = 0, writeamp = 0;
      if (readB) readamp = 100.0 * devicerb / readB;
      if (writeB) writeamp = 100.0 * devicewb / writeB;

      fprintf(stderr," IOPS / %zd), total %.2lf GB (R %.0lf%% W %.0lf%%)\n", (writeIOPS == 0) ? 0 : (writeB) / (writeIOPS), TOGB(trb + twb), readamp, writeamp);

      if (fp) {
	fprintf(fp, "%2.0lf\t%lf\t%.1lf\t%zd\t%.1lf\t%zd\t%.0lf\t%.0lf\n", elapsed, thistime, TOMB(readB), readIOPS, TOMB(writeB), writeIOPS, readamp, writeamp);
	fflush(fp);
      }

      if (threadContext->exitIOPS) {
	if (writeIOPS + readIOPS < threadContext->exitIOPS) {
	  exitcount++;
	  if (exitcount >= 30) {
	    fprintf(stderr,"*warning* early exiting due to %zd periods of low IOPS (%zd < %zd)\n", exitcount, writeIOPS + readIOPS, threadContext->exitIOPS);
	    keepRunning = 0;
	    break;
	  }
	}
      }

      last_trb = trb;
      last_tri = tri;
      last_twb = twb;
      last_twi = twi;

      
      i++;
    }

    if (thistime > starttime + threadContext->finishtime + 180) {
      fprintf(stderr,"*error* still running! watchdog termination (%.0lf > %.0lf + %zd\n", thistime, starttime, threadContext->finishtime + 180);
      keepRunning = 0;
      //      exit(-1);
    }
  }
  if (fp) {
    fclose(fp);
  }
  //  diskStatFree(&d);
  //  fprintf(stderr,"finished thread timer\n");
  keepRunning = 0;
  return NULL;
}



void jobRunThreads(jobType *job, const int num, const size_t maxSizeInBytes,
		   const size_t timetorun, const size_t dumpPos, char *benchmarkName, const size_t origqd,
		   unsigned short seed, int savePositions, diskStatType *d) {
  pthread_t *pt;
  CALLOC(pt, num+1, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num+1, sizeof(threadInfoType));

  keepRunning = 1;

  if (seed == 0) {
    seed = (unsigned short)timedouble();
    if (verbose) fprintf(stderr,"*info* setting seed based on the time to %d\n", seed);
  }

  positionContainer **allThreadsPC;
  CALLOC(allThreadsPC, num, sizeof(positionContainer*));

  const double thetime = timedouble();
  const size_t UUID = thetime * 10;

  
  for (size_t i = 0; i < num; i++) {
    threadContext[i].runTime = timetorun;
    threadContext[i].finishtime = timetorun;
    threadContext[i].exitIOPS = 0;

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
    

    {
      char *charI = strchr(job->strings[i], 'I');
      
      if (charI && *(charI + 1)) {
	threadContext[0].exitIOPS = atoi(charI + 1);
	fprintf(stderr,"*info* exit IOPS set to %zd\n", threadContext[0].exitIOPS);
      }
    }
    



    size_t runXtimes = 0;
    {
      char *charX = strchr(job->strings[i], 'X');
      
      if (charX && *(charX+1)) {
	runXtimes = atoi(charX + 1);
	threadContext[i].runTime = -1;
	threadContext[i].finishtime = -1;
					   
      }
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
    if ((verbose || (fitinram < mp)) && (i == 0)) {
      fprintf(stderr,"*info* using %.3lf GiB RAM for positions, we can store ", TOGiB(useRAM));
      commaPrint0dp(stderr, fitinram);
      fprintf(stderr," positions\n");
    }
    
    size_t countintime = mp;
    if ((long)timetorun > 0) { // only limit based on time if the time is positive
      
#define ESTIMATEIOPS 500000
      
      countintime = timetorun * ESTIMATEIOPS;
      if ((verbose || (countintime < mp)) && (i == 0)) {
	fprintf(stderr,"*info* in %zd seconds, at %d a second, would have at most ", timetorun, ESTIMATEIOPS);
	commaPrint0dp(stderr, countintime);
	fprintf(stderr," positions\n");
      }
    }
    size_t sizeLimitCount = (size_t)-1;
    if (limit != (size_t)-1) {
      sizeLimitCount = limit / avgBS;
      if (i == 0) {
	fprintf(stderr,"*info* to limit sum of lengths to %.1lf GiB, with avg size of %zd, requires %zd positions\n", TOGiB(limit), avgBS, sizeLimitCount);
      }
    }

    size_t mp2 = MIN(sizeLimitCount, MIN(countintime, MIN(mp, fitinram)));
    if (mp2 != mp) {
      mp = mp2;
      if (i == 0) {
	fprintf(stderr,"*info* positions limited to ");
	commaPrint0dp(stderr, mp);
	fprintf(stderr,"\n");
      }
    }
      
    threadContext[i].id = i;
    if (i == 0) {
      threadContext[i].benchmarkName = benchmarkName;
    } else {
      threadContext[i].benchmarkName = NULL;
    }
    threadContext[i].UUID = UUID;
    threadContext[i].ignoreResults = 0;
    positionContainerInit(&threadContext[i].pos, threadContext[i].UUID);
    threadContext[i].jobstring = job->strings[i];
    threadContext[i].jobdevice = job->devices[i];
    threadContext[i].waitfor = 0;
    threadContext[i].prewait = 0;
    threadContext[i].blockSize = bs;
    threadContext[i].highBlockSize = highbs;
    threadContext[i].rerandomize = 0;
    threadContext[i].runXtimes = runXtimes;

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

    {
      char *sf = strchr(job->strings[i], 'p');
      if (sf && *(sf+1)) {
	rw = atof(sf + 1);
      }
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
    //    if (seqFiles == 0) multiply = 3;

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



    {
      char *iTO = strchr(job->strings[i], 'T');
      if (iTO) {
	threadContext[i].runTime = atoi(iTO+1);
      }
    }

    {
      char *iTO = strchr(job->strings[i], 't');
      if (iTO) {
	threadContext[i].finishtime = atoi(iTO+1);
      }
    }


    {
      char *oOS = strchr(job->strings[i], 'O');
      if (oOS) {
	threadContext[i].runXtimes = 1;
	threadContext[i].runTime = -1;
      }
    }



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

    int qDepth = origqd;
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

      if (verbose >= 2) {
	checkPositionArray(threadContext[i].pos.positions, threadContext[i].pos.sz, threadContext[i].bdSize, !metaData);
      }
      
      threadContext[i].pos = positionContainerMultiply(&threadContext[i].pos, multiply);


      
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
  threadContext[0].pos.diskStats = d;
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


  if (savePositions) {
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
    
    
    positionContainerFree(&mergedpc);
    free(origpc);
  }

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

size_t jobCount(jobType *job) {
  return job->count;
}

size_t jobRunPreconditions(jobType *preconditions, const size_t count, const size_t maxSizeBytes) {
  if (count) {
    size_t gSize = 0;
    size_t coverage = 1;
    
    for (size_t i = 0; i < count; i++) {
      fprintf(stderr,"*info* precondition %zd: device '%s', command '%s'\n", i+1, preconditions->devices[i], preconditions->strings[i]);
      {
	char *charG = strchr(preconditions->strings[i], 'G');
	if (charG && *(charG+1)) {
	  // a G num is specified
	  gSize = 1024 * (size_t)(atof(charG + 1) * 1024 * 1024);
	}
      }
      
      size_t seqFiles = 0;
      {// seq or random
	char *charG = strchr(preconditions->strings[i], 's');
	if (charG && *(charG+1)) {
	  seqFiles = atoi(charG + 1);
	}
      }
      
      size_t exitIOPS = 2500; // 10MB/s lowerbound
      {// seq or random
	char *charG = strchr(preconditions->strings[i], 'I');
	if (charG && *(charG+1)) {
	  exitIOPS = atoi(charG + 1);
	}
      }

      if (gSize == 0) {
	char *charT = strchr(preconditions->strings[i], 'T');
	if (charT && *(charT+1)) {
	  // a T num is specified
	  gSize = 1024 * (size_t)(atof(charT + 1) * 1024 * 1024 * 1024);
	}
      }
      
      if (gSize) {
	coverage = (size_t) (ceil(gSize / 1.0 / maxSizeBytes));
      }
      
      char s[100];
      sprintf(s, "wk4s%zdG%.1lfX%zdx1nI%zd", seqFiles, (size_t)(maxSizeBytes / 1024.0 / 1024) / 1024.0, coverage, exitIOPS);
      free(preconditions->strings[i]);
      preconditions->strings[i] = strdup(s);
    }
    jobRunThreads(preconditions, count, maxSizeBytes, -1, 0, NULL, 256, 0 /*seed*/, 0 /*save positions*/, NULL); 
    fprintf(stderr,"*info* preconditioning complete\n");
  }
  return 0;
}
    
