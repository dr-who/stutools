#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>

#include <pthread.h>

#include "jobType.h"
#include "positions.h"
#include "utils.h"

#include "aioRequests.h"
#include "diskStats.h"

extern int keepRunning;

void jobInit(jobType *j) {
  j->count = 0;
  j->strings = NULL;
  j->devices = NULL;
}

void jobAdd(jobType *j, char *device, char *jobstring) {
  j->strings = realloc(j->strings, (j->count+1) * sizeof(char*));
  j->devices = realloc(j->devices, (j->count+1) * sizeof(char*));
  j->strings[j->count] = strdup(jobstring);
  j->devices[j->count] = strdup(device);
  j->count++;
}

void jobMultiply(jobType *j, const size_t extrajobs) {
  const int origcount = j->count;
  for (size_t i = 0; i < origcount; i++) {
    for (size_t n = 0; n < extrajobs; n++) {
      jobAdd(j, strdup(j->devices[i]), strdup(j->strings[i]));
    }
  }
}
  

void jobDump(jobType *j) {
  for (size_t i = 0; i < j->count; i++) {
    fprintf(stderr,"*info* job %zd, device %s, string %s\n", i, j->devices[i], j->strings[i]);
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
  positionContainer pos;
  size_t bdSize;
  size_t timetorun;
  size_t waitfor;
  char *jobstring;
  char *jobdevice;
  int dumpPositions;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  fprintf(stderr,"*info* thread[%zd] job is '%s'\n", threadContext->id, threadContext->pos.string);


  //  threadContext->pos = createPositions(threadContext->mp);

  //  setupPositions(threadContext->pos, &threadContext->mp, 0, 1, 4096, 4096, 4096, 0, threadContext->bdSize, NULL, threadContext->id);

  if (threadContext->dumpPositions) {
    dumpPositions(threadContext->pos.positions, threadContext->pos.string, threadContext->pos.sz, threadContext->dumpPositions);
  }

  logSpeedType benchl;
  logSpeedInit(&benchl);

  char *randomBuffer;
  CALLOC(randomBuffer, 4096, 1);
  memset(randomBuffer, 0, 4096);
  generateRandomBufferCyclic(randomBuffer, 4096, 0, 4096);

  size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
  int fd ;
  if (strchr(threadContext->jobstring,'w')) {
    fd = open(threadContext->jobdevice, O_RDWR | O_DIRECT);
  } else {
    fd = open(threadContext->jobdevice, O_RDONLY | O_DIRECT);
  }
  if (fd < 0) {
    perror(threadContext->jobdevice); return 0;
  }

  
  if (threadContext->waitfor) {
    fprintf(stderr,"*info* thread[%zd] waiting for %zd seconds\n", threadContext->id, threadContext->waitfor);
    sleep(threadContext->waitfor);
  }


  aioMultiplePositions(threadContext->pos.positions, threadContext->pos.sz, threadContext->timetorun, 256, -1, 0, NULL, &benchl, randomBuffer, 4096, 4096, &ios, &shouldReadBytes, &shouldWriteBytes, 0, 0, fd);

  close(fd);

  //  char name[1000];
  //  sprintf(name,"bob.%zd", threadContext->id);
  //  positionContainerSave(&threadContext->pos, name, 0);
  


  free(randomBuffer);

  logSpeedFree(&benchl);

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

  double start = timedouble(), last = start, thistime;
  while (keepRunning) {
    sleep(1);
    
    diskStatFinish(&d);
    size_t trb = 0, twb = 0;
    double util = 0;
    thistime = timedouble();
    double elapsed = thistime - start;
    diskStatSummary(&d, &trb, &twb, &util, 0, 0, 0, thistime - last);

    fprintf(stderr,"[%.1lf] read %.0lf MiB/s, write %.0lf MiB/s, util %.0lf %%\n", elapsed, TOMiB(trb), TOMiB(twb), util);
    last = thistime;
    
    i++;
    diskStatStart(&d);
  }
  return NULL;
}



void jobRunThreads(jobType *j, const int num, const size_t maxSizeInBytes, const size_t lowbs,
		   const size_t timetorun, const size_t dumpPositions) {
  pthread_t *pt;
  CALLOC(pt, num+1, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num+1, sizeof(threadInfoType));

  for (size_t i = 0; i < num; i++) {

    //    size_t fs = fileSizeFromName(j->devices[i]);
    threadContext[i].bdSize = maxSizeInBytes;
    size_t mp = (size_t) (threadContext[i].bdSize / lowbs);
    //    fprintf(stderr,"file size %zd, positions %zd\n", fs, mp);
    if (maxSizeInBytes) {
      if (maxSizeInBytes < mp) {
	mp = maxSizeInBytes;
      }
    }
      
    threadContext[i].id = i;
    threadContext[i].jobstring = strdup(j->strings[i]);
    threadContext[i].jobdevice = strdup(j->devices[i]);
    positionContainerInit(&threadContext[i].pos);
    threadContext[i].timetorun = timetorun;
    threadContext[i].waitfor = 0;
    threadContext[i].dumpPositions = dumpPositions;

    // do this here to allow repeatable random numbers
    float rw = 0.5;
    if (strchr(j->strings[i], 'r')) {
      rw += 0.5;
    } else if (strchr(j->strings[i], 'w')) {
      rw -= 0.5;
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
      size_t newmp = atoi(pChar + 1);
      if (newmp < mp) {
	mp = newmp;
      }
    }

    char *Wchar = strchr(j->strings[i], 'W');
    if (Wchar) {
      size_t waitfor = atoi(Wchar + 1);
      threadContext[i].waitfor = waitfor;
      threadContext[i].timetorun -= waitfor;
      if (threadContext[i].timetorun < 0) {
	threadContext[i].timetorun = 0;
      }
    }

    positionContainerSetup(&threadContext[i].pos, mp, j->devices[i], j->strings[i]);
    setupPositions(threadContext[i].pos.positions, &mp, seqFiles, rw, bs, bs, bs, -99999, threadContext[i].bdSize, NULL, threadContext[i].id);
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


  
