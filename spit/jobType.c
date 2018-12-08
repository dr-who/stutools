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
    free(job->strings[i]);
    free(job->devices[i]);
  }
  free(job->strings);
  free(job->devices);
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
  size_t queueDepth;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  if (verbose >= 2) {
    fprintf(stderr,"*info* thread[%zd] job is '%s'\n", threadContext->id, threadContext->pos.string);
  }


  //  threadContext->pos = createPositions(threadContext->mp);

  //  setupPositions(threadContext->pos, &threadContext->mp, 0, 1, 4096, 4096, 4096, 0, threadContext->bdSize, NULL, threadContext->id);


  logSpeedType benchl;
  logSpeedInit(&benchl);

  char *randomBuffer;
  CALLOC(randomBuffer, threadContext->blockSize, 1);
  memset(randomBuffer, 0, threadContext->blockSize);
  generateRandomBufferCyclic(randomBuffer, threadContext->blockSize, 0, 4096);

  size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
  int fd ;
  int direct = O_DIRECT;
  if (strchr(threadContext->jobstring, 'D')) {
    fprintf(stderr,"*info* thread[%zd] turning off O_DIRECT\n", threadContext->id);
    direct = 0; // don't use O_DIRECT if the user specifes 'D'
  }

  if (strchr(threadContext->jobstring,'r')) {
    fd = open(threadContext->jobdevice, O_RDONLY | direct);
  } else {
    fd = open(threadContext->jobdevice, O_RDWR | direct);
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


  fprintf(stderr,"*info* [thread %zd] starting '%s' with ", threadContext->id, threadContext->jobstring);
  commaPrint0dp(stderr, threadContext->pos.sz);
  fprintf(stderr," positions, qd=%zd\n", threadContext->queueDepth);
  
  aioMultiplePositions(threadContext->pos.positions, threadContext->pos.sz, threadContext->finishtime, threadContext->queueDepth, -1, 0, NULL, &benchl, randomBuffer, threadContext->blockSize, threadContext->blockSize, &ios, &shouldReadBytes, &shouldWriteBytes, 0, 1, fd);
  fprintf(stderr,"*info [thread %zd] finished '%s' with %zd positions\n", threadContext->id, threadContext->jobstring, threadContext->pos.sz);
  close(fd);

  //  char name[1000];
  //  sprintf(name,"bob.%zd", threadContext->id);
  //  positionContainerSave(&threadContext->pos, name, 0);
  


  free(randomBuffer);

  logSpeedFree(&benchl);
  //  fprintf(stderr,"finished thread %zd\n", threadContext->id);

  return NULL;
}



static void *runThreadTimer(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;

  size_t i = 0;
  diskStatType d;
  diskStatSetup(&d);
  int fd = open(threadContext->jobdevice, O_RDONLY);
  if (fd < 0) {
    perror("diskstats"); return NULL;
  }
  diskStatAddDrive(&d, fd);
  close(fd);
  
  diskStatStart(&d);

  double start = timedouble(), last = start, thistime = start;
  while (timedouble() < threadContext->finishtime) {
    sleep(1);
    
    diskStatFinish(&d);
    size_t trb = 0, twb = 0, tri = 0, twi = 0;
    double util = 0;
    thistime = timedouble();
    double elapsed = thistime - start;
    diskStatSummary(&d, &trb, &twb, &tri, &twi, &util, 0, 0, 0, thistime - last);

    fprintf(stderr,"[%2.0lf] read ", elapsed);
    commaPrint0dp(stderr, TOMiB(trb));
    fprintf(stderr," MiB/s (");
    commaPrint0dp(stderr, tri);
    fprintf(stderr," IOPS), write ");
    commaPrint0dp(stderr, TOMiB(twb));
    fprintf(stderr," MiB/s (");
    commaPrint0dp(stderr, twi);
    fprintf(stderr," IOPS), util %.0lf %%\n", util);

    //    fprintf(stderr,"[%2.0lf] read %.0lf MiB/s (%zd IOPS), write %.0lf MiB/s (%zd IOPS), util %.0lf %%\n", elapsed, TOMiB(trb), tri, TOMiB(twb), twi, util);

    last = thistime;
    
    i++;
    diskStatStart(&d);
  }
  diskStatFree(&d);
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

  double currenttime = timedouble();
  double finishtime = currenttime + timetorun;
  assert (finishtime >= currenttime + timetorun);
  keepRunning = 1;
  
  for (size_t i = 0; i < num; i++) {

    int bs = 4096, highbs = 4096;
    char *charBS = strchr(job->strings[i], 'k');
    if (charBS && *(charBS+1)) {

      char *endp = NULL;
      bs = 1024 * strtol(charBS+1, &endp, 10);
      if (*endp != 0) {
	int nextv = atoi(endp+1);
	if (nextv > 0) {
	  highbs = 1024 * nextv;
	}
      }
      if (highbs < bs) {
	highbs = bs;
      }
      if (verbose >= 2) {
	fprintf(stderr,"*info* setting blockSize to be [%d, %d]\n", bs, highbs);
      }
    }

    
    //    size_t fs = fileSizeFromName(job->devices[i]);
    threadContext[i].bdSize = maxSizeInBytes;
    size_t mp = (size_t) (threadContext[i].bdSize / bs);
    //    fprintf(stderr,"file size %zd, positions %zd\n", fs, mp);
    size_t fitinram = totalRAM() / 4 / num / sizeof(positionType);
    //    fprintf(stderr,"fit into ram %zd, currently %zd\n", fitinram, mp);
    if (mp > fitinram) {
      mp = fitinram;
      if (verbose >= 2) {
	fprintf(stderr,"*warning* limited to %zd because of RAM\n", mp);
      }
    }
      
    threadContext[i].id = i;
    threadContext[i].jobstring = job->strings[i];
    threadContext[i].jobdevice = job->devices[i];
    positionContainerInit(&threadContext[i].pos);
    threadContext[i].finishtime = finishtime;
    threadContext[i].waitfor = 0;
    threadContext[i].blockSize = bs;

    // do this here to allow repeatable random numbers
    float rw = 0.5;
    if (strchr(job->strings[i], 'r')) {
      rw += 0.5;
    }
    if (strchr(job->strings[i], 'w')) {
      rw -= 0.5;
    }
    
    int seqFiles = 1;
    {
      char *sf = strchr(job->strings[i], 's');
      if (sf && *(sf+1)) {
	seqFiles = atoi(sf+1);
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
	size_t newmp = atoi(pChar + 1);
	if (newmp < mp) {
	  mp = newmp;
	}
      }
    }

    size_t seed = i;
    {
      char *RChar = strchr(job->strings[i], 'R');
      if (RChar && *(RChar+1)) {
	size_t seed = atoi(RChar + 1);
	srand48(seed);
      }
    }

    char *Wchar = strchr(job->strings[i], 'W');
    if (Wchar && *(Wchar+1)) {
      size_t waitfor = atoi(Wchar + 1);
      if (waitfor > timetorun) {
	waitfor = timetorun;
	fprintf(stderr,"*warning* waiting decreased to %zd\n", waitfor);
      }
      
      threadContext[i].waitfor = waitfor;
    }

    positionContainerSetup(&threadContext[i].pos, mp, job->devices[i], job->strings[i]);
    setupPositions(threadContext[i].pos.positions, &threadContext[i].pos.sz, seqFiles, rw, bs, highbs, bs, -99999, threadContext[i].bdSize, NULL, seed);
    if (checkPositionArray(threadContext[i].pos.positions, mp, threadContext[i].bdSize)) {
      abort();
    }

    if (dumpPositionsN) {
      dumpPositions(threadContext[i].pos.positions, threadContext[i].pos.string, threadContext[i].pos.sz, dumpPositionsN);
    }
    
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
  // now wait for the timer thread (probably don't need this)
  pthread_join(pt[num], NULL);

  // print stats and free
  for (size_t i = 0; i < num; i++) {
    positionLatencyStats(&threadContext[i].pos, i);
    positionContainerFree(&threadContext[i].pos);
  }


  free(threadContext);
  free(pt);
}


void jobAddDeviceToAll(jobType *job, const char *device) {
  for (size_t i = 0; i < job->count; i++) {
    job->devices[i] = strdup(device);
  }
}
    
