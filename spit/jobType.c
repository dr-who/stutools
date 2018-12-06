#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <pthread.h>

#include "jobType.h"
#include "positions.h"
#include "utils.h"
#include "devices.h"

#include "aioRequests.h"
#include "diskStats.h"

extern int keepRunning;

void jobInit(jobType *j) {
  j->count = 0;
  j->strings = NULL;
}

void jobAdd(jobType *j, char *str) {
  j->strings = realloc(j->strings, (j->count+1) * sizeof(char*));
  j->strings[j->count++] = strdup(str);
}

void jobDump(jobType *j) {
  for (size_t i = 0; i < j->count; i++) {
    fprintf(stderr,"*info* job %zd, string %s\n", i, j->strings[i]);
  }
}

void jobDumpAll(jobType *j) {
  for (size_t i = 0; i < j->count; i++) {
    fprintf(stderr,"*info* job %zd, string %s\n", i, j->strings[i]);
  }
}

void jobFree(jobType *j) {
  for (size_t i = 0; i < j->count; i++) {
    free(j->strings[i]);
  }
  free(j->strings);
  jobInit(j);
}

typedef struct {
  size_t id;
  deviceDetails *dev;
  positionContainer pos;
  size_t bdSize;
  size_t timetorun;
  size_t waitfor;
  char *jobstring;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  fprintf(stderr,"*info* thread id %zd, job is '%s'\n", threadContext->id, threadContext->pos.string);


  if (threadContext->waitfor) {
    fprintf(stderr,"*info* thread %zd waiting for %zd seconds\n", threadContext->id, threadContext->waitfor);
    sleep(threadContext->waitfor);
  }

  //  threadContext->pos = createPositions(threadContext->mp);

  //  setupPositions(threadContext->pos, &threadContext->mp, 0, 1, 4096, 4096, 4096, 0, threadContext->bdSize, NULL, threadContext->id);

  //  dumpPositions(threadContext->pos.positions, threadContext->pos.string, threadContext->pos.sz, 10);

  logSpeedType benchl;
  logSpeedInit(&benchl);

  char *randomBuffer;
  CALLOC(randomBuffer, 4096, 1);
  memset(randomBuffer, 0, 4096);
  generateRandomBufferCyclic(randomBuffer, 4096, 0, 4096);

  size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
  aioMultiplePositions(threadContext->pos.positions, threadContext->pos.sz, threadContext->timetorun, 256, -1, 0, NULL, &benchl, randomBuffer, 4096, 4096, &ios, &shouldReadBytes, &shouldWriteBytes, 0, 0, threadContext->pos.dev);

  char name[1000];
  sprintf(name,"bob.%zd", threadContext->id);
  positionContainerSave(&threadContext->pos, name, 0);
  


  free(randomBuffer);

  logSpeedFree(&benchl);

  return NULL;
}



static void *runThreadTimer(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;

  size_t i = 0;
  diskStatType d;
  diskStatSetup(&d);
  diskStatAddDrive(&d, threadContext->dev->fd);
  diskStatStart(&d);

  double start = timedouble(), last = start, thistime;
  while (keepRunning) {
    sleep(1);
    
    diskStatFinish(&d);
    size_t trb = 0, twb = 0;
    double util = 0;
    thistime = timedouble();
    double elapsed = thistime - start;
    diskStatSummary(&d, &trb, &twb, &util, 0, 0, 0, thistime - last);

    fprintf(stderr,"[%.1lf] trb %.1lf, twb %.1lf, util %.3lf\n", elapsed, TOMiB(trb), TOMiB(twb), util);
    last = thistime;
    
    i++;
    diskStatStart(&d);
  }
  return NULL;
}



void jobRunThreads(jobType *j, const int num, const size_t maxSizeInBytes, const size_t lowbs, deviceDetails *dev, size_t timetorun) {
  pthread_t *pt;
  CALLOC(pt, num+1, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num+1, sizeof(threadInfoType));

  size_t maxPositions = (int)(maxSizeInBytes / lowbs);
  for (size_t i = 0; i < num; i++) {
    threadContext[i].id = i;
    threadContext[i].dev = dev;
    threadContext[i].jobstring = j->strings[i];
    positionContainerInit(&threadContext[i].pos);
    threadContext[i].bdSize = maxSizeInBytes;
    threadContext[i].timetorun = timetorun;
    threadContext[i].waitfor = 0;

    // do this here to allow repeatable random numbers
    float rw = 0.5;
    if (strchr(j->strings[i], 'r')) {
      rw += 0.5;
    } else if (strchr(j->strings[i], 'w')) {
      rw -= 0.0;
    }

    int seqFiles = 1;
    char *sf = strchr(j->strings[i], 's');
    if (sf) {
      seqFiles = atoi(sf+1);
    }

    int bs = 4096;
    char *bschar = strchr(j->strings[i], 'k');
    if (bschar) {
      bs = 1024 * atoi(bschar + 1);
    }

    char *pChar = strchr(j->strings[i], 'P');
    if (pChar) {
      maxPositions = 1024 * atoi(pChar + 1);
    }

    char *Wchar = strchr(threadContext->jobstring, 'W');
    if (Wchar) {
      size_t waitfor = atoi(Wchar + 1);
      threadContext[i].waitfor = waitfor;
  }


    positionContainerSetup(&threadContext[i].pos, maxPositions, dev, j->strings[i]);
    setupPositions(threadContext[i].pos.positions, &maxPositions, seqFiles, rw, bs, bs, bs, 0, threadContext[i].bdSize, NULL, threadContext[i].id);
  }
  
    
  pthread_create(&(pt[num]), NULL, runThreadTimer, &(threadContext[0]));
  for (size_t i = 0; i < num; i++) {
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }


  for (size_t i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
    positionContainerFree(&threadContext[i].pos);
  }

  free(threadContext);
  free(pt);
}


  
