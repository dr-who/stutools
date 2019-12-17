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
#include "histogram.h"
#include "blockVerify.h"

extern volatile int keepRunning;
extern int verbose;

void jobInit(jobType *job) {
  job->count = 0;
  job->strings = NULL;
  job->devices = NULL;
  job->deviceid = NULL;
  job->delay = NULL;
}

void jobAddBoth(jobType *job, char *device, char *jobstring) {
  job->strings = realloc(job->strings, (job->count+1) * sizeof(char*));
  job->devices = realloc(job->devices, (job->count+1) * sizeof(char*));
  job->deviceid = realloc(job->deviceid, (job->count+1) * sizeof(int));
  job->delay = realloc(job->delay, (job->count+1) * sizeof(double));
  job->strings[job->count] = strdup(jobstring);
  job->devices[job->count] = strdup(device);
  int deviceid = job->count;
  for (int i = 0; i < job->count; i++) {
    if (strcmp(device, job->devices[i])==0) {
      deviceid = i;
      break;
    }
  }
  job->deviceid[job->count] = deviceid;
  job->count++;
}

void jobAdd3(jobType *job, const char *jobstring, const double delay) {
  job->strings = realloc(job->strings, (job->count+1) * sizeof(char*));
  job->devices = realloc(job->devices, (job->count+1) * sizeof(char*));
  job->deviceid = realloc(job->deviceid, (job->count+1) * sizeof(int));
  job->delay = realloc(job->delay, (job->count+1) * sizeof(double));
  job->strings[job->count] = strdup(jobstring);
  job->devices[job->count] = NULL;
  job->deviceid[job->count] = 0;
  job->delay[job->count] = delay;
  job->count++;
}


void jobAdd(jobType *job, const char *jobstring) {
  jobAdd3(job, jobstring, 0);
}


void jobAddExec(jobType *job, const char *jobstring, const double delay) {
  jobAdd3(job, jobstring, delay);
}

void jobMultiply(jobType *job, const size_t extrajobs, deviceDetails *deviceList, size_t deviceCount) {
  const int origcount = job->count;
  for (int i = 0; i < origcount; i++) {
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
  


void jobFileSequence(jobType *job) {
  const int origcount = job->count;
  for (int i = 0; i < origcount; i++) {
    char s[1000];
    sprintf(s, "%s.%04d", job->devices[i], i+1);
    free(job->devices[i]);
    job->devices[i] = strdup(s);
    job->deviceid[i] = i;
  }
}
  

void jobDump(jobType *job) {
  fprintf(stderr,"*info* jobDump: %zd\n", jobCount(job));
  for (int i = 0; i < job->count; i++) {
    fprintf(stderr,"*  info* job %d, device %s, deviceid %d, delay %.g, string %s\n", i, job->devices[i], job->deviceid[i], job->delay[i], job->strings[i]);
  }
}

void jobDumpAll(jobType *job) {
  for (int i = 0; i < job->count; i++) {
    fprintf(stderr,"*info* job %d, string %s\n", i, job->strings[i]);
  }
}

void jobFree(jobType *job) {
  for (int i = 0; i < job->count; i++) {
    //    fprintf(stderr,"freeinging %d of %d\n", i, job->count);
    if (job->strings[i]) {free(job->strings[i]); job->strings[i] = NULL;}
    if (job->devices[i]) {free(job->devices[i]); job->devices[i] = NULL;}
  }
  free(job->strings); job->strings = NULL;
  free(job->devices); job->devices = NULL;
  free(job->delay); job->delay = NULL;
  jobInit(job);
}

typedef struct {
  size_t id;
  positionContainer pos;
  size_t minbdSize;
  size_t maxbdSize;
  size_t minSizeInBytes;
  size_t maxSizeInBytes;
  size_t finishTime; // the overall thread finish time
  size_t waitfor;
  size_t prewait;
  size_t exec;
  char *jobstring;
  char *jobdevice;
  int jobdeviceid;
  size_t uniqueSeeds;
  size_t verifyUnique;
  size_t dumpPos;
  size_t blockSize;
  size_t highBlockSize;
  size_t queueDepth;
  size_t metaData;
  size_t QDbarrier;
  size_t flushEvery;
  double rw;
  size_t startingBlock;
  size_t mod;
  size_t remain;
  int rerandomize; 
  int addBlockSize;
  size_t runXtimesTI;
  size_t multipleTimes;
  unsigned short seed;
  size_t speedMB;
  char *randomBuffer;
  size_t numThreads;
  size_t waitForThreads;
  size_t *go;
  pthread_mutex_t *gomutex;
  positionContainer **allPC;
  char *benchmarkName;
  size_t anywrites;
  size_t UUID;
  size_t seqFiles;
  size_t seqFilesMaxSizeBytes;
  size_t jumbleRun;
  size_t runTime; // the time for a round
  size_t ignoreResults;
  size_t exitIOPS;
  double timeperline;
  double ignorefirst;
  char *mysqloptions;
  char *mysqloptions2;
  char *commandstring;
  char *filePrefix;
  size_t o_direct;
  lengthsType len;
} threadInfoType;


static void *runThread(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  if (verbose >= 2) {
    fprintf(stderr,"*info* thread[%zd] job is '%s', size = %zd\n", threadContext->id, threadContext->jobstring, threadContext->pos.sz);
  }




  // allocate the position array space
  // create the positions and the r/w status
  //    threadContext->seqFiles = seqFiles;
  //    threadContext->seqFilesMaxSizeBytes = seqFilesMaxSizeBytes;
  positionContainerCreatePositions(&threadContext->pos, threadContext->jobdeviceid, threadContext->seqFiles, threadContext->seqFilesMaxSizeBytes, threadContext->rw, &threadContext->len, MIN(4096,threadContext->blockSize), threadContext->startingBlock, threadContext->minbdSize, threadContext->maxbdSize, threadContext->seed, threadContext->mod, threadContext->remain);
  
  if (verbose >= 2) {
    positionContainerCheck(&threadContext->pos, threadContext->minbdSize, threadContext->maxbdSize, !threadContext->metaData);
  }
  
  if (threadContext->seqFiles == 0) positionContainerRandomize(&threadContext->pos);
  
  if (threadContext->jumbleRun) positionContainerJumble(&threadContext->pos, threadContext->jumbleRun);
  
  //      positionPrintMinMax(threadContext->pos.positions, threadContext->pos.sz, threadContext->minbdSize, threadContext->maxbdSize, threadContext->minSizeInBytes, threadContext->maxSizeInBytes);
  threadContext->anywrites = (threadContext->rw < 1); 
  calcLBA(&threadContext->pos); // calc LBA coverage
  
  
  if (threadContext->uniqueSeeds) {
    positionContainerUniqueSeeds(&threadContext->pos, threadContext->seed, threadContext->verifyUnique);
  } else if (threadContext->metaData) {
    positionContainerAddMetadataChecks(&threadContext->pos);
  }
  
  if (threadContext->dumpPos /* && !iRandom*/) {
    positionContainerDump(&threadContext->pos, threadContext->dumpPos);
  }
  
  
  CALLOC(threadContext->randomBuffer, threadContext->highBlockSize, 1);
  memset(threadContext->randomBuffer, 0, threadContext->highBlockSize);
  generateRandomBufferCyclic(threadContext->randomBuffer, threadContext->highBlockSize, threadContext->seed, threadContext->highBlockSize);

  size_t ios = 0, shouldReadBytes = 0, shouldWriteBytes = 0;
  int fd,  direct = O_DIRECT;
  if (strchr(threadContext->jobstring, 'D')) {
    fprintf(stderr,"*info* thread[%zd] turning off O_DIRECT\n", threadContext->id);
    direct = 0; // don't use O_DIRECT if the user specifes 'D'
  }
  threadContext->o_direct = direct;

  // check block sizes
  {
    char *suffix = getSuffix(threadContext->jobdevice);
    size_t phybs, logbs;
    getPhyLogSizes(suffix, &phybs, &logbs);
    //    fprintf(stderr,"*info* device %s, physical io size %zd, logical io size %zd\n", threadContext->jobdevice, phybs, logbs);
    free(suffix);

    for (int i = 0; i < (int)threadContext->pos.sz; i++) {
      if (threadContext->pos.positions[i].len < logbs) {
	fprintf(stderr,"*error* device '%s' doesn't support size %d [logical bs %zd]\n", threadContext->jobdevice, threadContext->pos.positions[i].len, logbs);
	exit(1);
      }
    }
  }
  
  
  if (threadContext->anywrites) {
    if (threadContext->filePrefix) {
      // always create new file on prefix and writes, delete existing
      fd = open(threadContext->jobdevice, O_RDWR | O_CREAT | O_TRUNC | direct, S_IRUSR | S_IWUSR);
    } else {
      // else just open
      fd = open(threadContext->jobdevice, O_RDWR | direct);
    }
    if (verbose >= 2) fprintf(stderr,"*info* open with O_RDWR\n");
  } else {
    fd = open(threadContext->jobdevice, O_RDONLY | direct);
    if (verbose >= 2) fprintf(stderr,"*info* open with O_RDONLY\n");
  }
    
  if (fd < 0) {
    fprintf(stderr,"*error* problem opening '%s' with direct=%d, writes=%zd\n", threadContext->jobdevice, direct, threadContext->anywrites);
    fprintf(stderr,"*info* some filesystems don't support direct, maybe add D to the command string.\n");
    perror(threadContext->jobdevice); exit(-1);
  }

  char *suffix = getSuffix(threadContext->jobdevice);

  if (verbose) {
    char *model = getModel(suffix);
    if (model && threadContext->id == 0) {
      fprintf(stderr,"*info* device: '%s'\n", model);
      free(model);
    }
  }
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

  if (!threadContext->exec && (threadContext->finishTime < threadContext->runTime)) {
    fprintf(stderr,"*warning* timing %zd > %zd doesn't make sense\n", threadContext->runTime, threadContext->finishTime);
  }
  //  double localrange = TOGB(threadContext->maxbdSize - threadContext->minbdSize);
  // minSizeInBytes is the -G range. If there are multiple threads carving the LBA up, it'll be specified as the min/max bdsize
  size_t outerrange = threadContext->maxbdSize - threadContext->minbdSize;
  size_t sumrange = 0;
  for (int i = 0; i < (int)threadContext->pos.sz; i++) {
    sumrange += threadContext->pos.positions[i].len;
  }
  if (verbose >= 1)
    fprintf(stderr,"*info* [t%zd] '%s %s' s%zd (%.0lf KiB), [%zd] (LBA %.0lf%%, [%.2lf,%.2lf]/%.2lf GiB), n=%d, q%zd (Q%zd) R/w=%.2g, F=%zd, k=[%.0lf-%.0lf], R=%u, B%zd W%zd T%zd t%zd x%zd X%zd\n", threadContext->id, threadContext->jobstring, threadContext->jobdevice, threadContext->seqFiles, TOKiB(threadContext->seqFilesMaxSizeBytes), threadContext->pos.sz, sumrange * 100.0 / outerrange, TOGiB(threadContext->minbdSize), TOGiB(threadContext->maxbdSize), TOGiB(outerrange), threadContext->rerandomize, threadContext->queueDepth, threadContext->QDbarrier, threadContext->rw, threadContext->flushEvery, TOKiB(threadContext->blockSize), TOKiB(threadContext->highBlockSize), threadContext->seed, threadContext->prewait, threadContext->waitfor, threadContext->runTime, threadContext->finishTime, threadContext->multipleTimes, threadContext->runXtimesTI);


  // do the mahi
  pthread_mutex_lock(threadContext->gomutex);
  (*threadContext->go)++;
  pthread_mutex_unlock(threadContext->gomutex);
  
  while(*threadContext->go < threadContext->waitForThreads) {
    usleep(10);
  }
  if (verbose >= 1)
    fprintf(stderr,"*info* starting thread %zd ('%s')\n", threadContext->id, threadContext->jobstring);

  
  double starttime = timedouble();
  if (threadContext->prewait) {
    fprintf(stderr,"*info* sleeping for %zd\n", threadContext->prewait);
    sleep(threadContext->prewait);
  }

  if (threadContext->exec) {
    char *comma = strchr(threadContext->jobstring, ',');
    if (comma && *(comma+1)) {
      char *env[] = {"/bin/bash", "-c", comma+1, NULL};
      fprintf(stderr,"*info* executing '%s'\n", threadContext->jobstring);
      runCommand("/bin/bash", env);
      return NULL;
    }
  }
  


  size_t iteratorCount = 0, iteratorInc = 1, iteratorMax = 0;

  size_t byteLimit = 0;
  if (threadContext->multipleTimes) {
    iteratorMax = 1;
    byteLimit = threadContext->multipleTimes * outerrange;
    if (threadContext->rerandomize || threadContext->addBlockSize) {
      iteratorMax = -1;
    }
  } else if (threadContext->runXtimesTI) {
    iteratorMax = threadContext->runXtimesTI;
    byteLimit = threadContext->runXtimesTI * sumrange;
    iteratorInc = threadContext->runXtimesTI;
    if (threadContext->rerandomize || threadContext->addBlockSize) {
      iteratorInc = 1; // just run once and then rerandomize
      iteratorMax = -1;
    }
  }
  
  if (threadContext->id == 0) {
    fprintf(stderr,"*info* byteLimit %zd (%.03lf GiB), iteratorInc %zd, iteratorMax %zd\n", byteLimit, TOGiB(byteLimit), iteratorInc, iteratorMax);
  }

  size_t totalB = 0, ioerrors = 0;
  while (keepRunning) {
    // 
    totalB += aioMultiplePositions(&threadContext->pos, threadContext->pos.sz, timedouble() + threadContext->runTime, byteLimit, threadContext->queueDepth, -1 /* verbose */, 0, /*threadContext->randomBuffer, threadContext->highBlockSize, */ MIN(4096,threadContext->blockSize), &ios, &shouldReadBytes, &shouldWriteBytes,  threadContext->rerandomize || threadContext->addBlockSize, 1, fd, threadContext->flushEvery, threadContext->speedMB, &ioerrors, threadContext->QDbarrier);

    // check exit constraints
    if (byteLimit) {
      if (totalB >= byteLimit) {
	break;
      }
    } else {
      if (timedouble() > starttime + threadContext->finishTime)
	break;
    }

    iteratorCount += iteratorInc;

    if (verbose >= 1) {
      if (!keepRunning && threadContext->id == 0) {fprintf(stderr,"*info* interrupted...\n");}
    }
    if (iteratorMax > 0) {
      if (iteratorCount >= iteratorMax) {
	break;
      }
    }
    if (keepRunning && (threadContext->rerandomize || threadContext->addBlockSize)) {
      if (threadContext->rerandomize) {
	if (verbose >= 2) fprintf(stderr,"*info* shuffling positions, 1st = %zd\n", threadContext->pos.positions[0].pos);
	positionContainerRandomize(&threadContext->pos);
      }
      if (threadContext->addBlockSize) {
	if (verbose >= 2) fprintf(stderr,"*info* adding %zd to all positions, 1st = %zd\n", threadContext->highBlockSize, threadContext->pos.positions[0].pos);
	positionAddBlockSize(threadContext->pos.positions, threadContext->pos.sz, threadContext->highBlockSize, threadContext->minbdSize, threadContext->maxbdSize);
      }	
    }
    sleep(threadContext->waitfor);
    if (verbose) fprintf(stderr,"*info* finished pass %zd\n", iteratorCount);
  }

  if (verbose) fprintf(stderr,"*info [thread %zd] finished '%s'\n", threadContext->id, threadContext->jobstring);
  threadContext->pos.elapsedTime = timedouble() - starttime;

  //  if (verbose) {fprintf(stderr,"*info* starting fdatasync()..."); fflush(stderr);}
  //  fdatasync(fd); // make sure all the data is on disk before we axe off the ioc
  //  if (verbose) {fprintf(stderr," finished\n"); fflush(stderr);}

  close(fd);

  if (ioerrors) {
    fprintf(stderr,"*warning* there were %zd IO errors\n", ioerrors);
    exit(-1);
  }

  return NULL;
}


char *getValue(const char *os, const char *prefix) {
  assert(prefix);
  if (!os) {
    //    abort();
    return strdup("NULL");
  }

  const int plen = strlen(prefix);
  
  char *i=strstr(os, prefix);
  if (!i) {
    return strdup("NULL");
  }
  char *k=strchr(i, ',');
  if (!k) {
    return strdup(i + plen);
  } else {
    char s[1000];
    strncpy(s, i + plen, k - i - plen);
    s[k-i-plen]=0;
    return strdup(s);
  }
}

static void *runThreadTimer(void *arg) {
  threadInfoType *threadContext = (threadInfoType*)arg;
  double TIMEPERLINE = threadContext->timeperline;
  //  if (TIMEPERLINE < 0.000001) TIMEPERLINE=0.000001;
  if (TIMEPERLINE <= 0) TIMEPERLINE = 0.00001;
  double ignorefirst = threadContext->ignorefirst;
  if (ignorefirst < 0) ignorefirst = 0;

  while(*threadContext->go < threadContext->waitForThreads) {
    usleep(10);
    //    fprintf(stderr,"."); fflush(stderr);
  }
  //  fprintf(stderr,"%zd %zd\n", *threadContext->go, threadContext->waitForThreads);
  
  if (verbose >= 2) {
    fprintf(stderr,"*info* timer thread, threads %zd, %.2lf per line, ignore first %.2lf seconds, %s, finish time %zd\n", threadContext->numThreads, TIMEPERLINE, ignorefirst, threadContext->benchmarkName, threadContext->finishTime);
  }

  double thistime = 0;
  volatile size_t last_trb = 0, last_twb = 0, last_tri = 0, last_twi = 0;
  volatile size_t trb = 0, twb = 0, tri = 0, twi = 0;


  FILE *fp = NULL, *fpmysql = NULL;
  char s[1000];
  if (threadContext->benchmarkName) {
    sprintf(s, "%s.my", threadContext->benchmarkName);
    fp = fopen(threadContext->benchmarkName, "wt");
    if (threadContext->mysqloptions) {
      fpmysql = fopen(s, "wt");
      if (!fpmysql) {
	perror(s);
      } else {
	fprintf(fpmysql, "# start of file\n");
	fflush(fpmysql);
      }
    }
    
    if (fp) {
      fprintf(stderr,"*info* benchmark file '%s' (MySQL '%s')\n", threadContext->benchmarkName, s);
    } else {
      fprintf(stderr,"*error* couldn't create benchmark file '%s' (MySQL '%s')\n", threadContext->benchmarkName, s);
      perror(threadContext->benchmarkName);
    }
  }

  double total_printed_r_bytes = 0, total_printed_w_bytes = 0, total_printed_r_iops = 0, total_printed_w_iops = 0;
  size_t exitcount = 0;
  double starttime = timedouble();
  double lasttime = starttime, nicetime = TIMEPERLINE;
  double lastprintedtime = lasttime;

  // grab the start, setup the "last" values
  for (size_t j = 0; j < threadContext->numThreads;j++) {
    assert(threadContext->allPC);
    if (threadContext->allPC && threadContext->allPC[j]) {
      last_tri += threadContext->allPC[j]->readIOs;
      last_trb += threadContext->allPC[j]->readBytes;
      
      last_twi += threadContext->allPC[j]->writtenIOs;
      last_twb += threadContext->allPC[j]->writtenBytes;
    }
  }

  double last_devicerb = 0, last_devicewb = 0;

  // loop until the time runs out
  int printlinecount = 0;
  while (keepRunning && ((thistime = timedouble()) < starttime + threadContext->finishTime + TIMEPERLINE)) {

    if ((thistime - starttime >= nicetime) && (thistime < starttime + threadContext->finishTime + TIMEPERLINE)) {
      printlinecount++;
      // find a nice time for next values
      if (threadContext->timeperline <= 0) { // if -s set to <= 0
	if (thistime - starttime < 0.1) { // 10ms for 1 s
	  TIMEPERLINE = 0.001;
	} else  if (thistime - starttime < 10) { // 10ms for 1 s
	  TIMEPERLINE = 0.01;
	} else if (thistime - starttime < 30) { // 10ms for up to 5 sec
	  TIMEPERLINE = 0.1;
	} else {
	  TIMEPERLINE = 1;
	}
      }
      nicetime = thistime - starttime + TIMEPERLINE;
      nicetime = (size_t)(floor(nicetime / TIMEPERLINE)) * TIMEPERLINE;

      trb = 0; twb = 0;  tri = 0;  twi = 0;
      
      double devicerb = 0, devicewb = 0;
      
	for (size_t j = 0; j < threadContext->numThreads;j++) {
	  assert(threadContext->allPC);
	  if (threadContext->allPC && threadContext->allPC[j]) {
	    tri += threadContext->allPC[j]->readIOs;
	    trb += threadContext->allPC[j]->readBytes;
	    
	    twi += threadContext->allPC[j]->writtenIOs;
	    twb += threadContext->allPC[j]->writtenBytes;
	  }
	}
	
	total_printed_r_bytes = trb;
	total_printed_r_iops =  tri;

	total_printed_w_bytes = twb;
	total_printed_w_iops =  twi;
	
	if (threadContext->pos.diskStats) {
	  diskStatFinish(threadContext->pos.diskStats);
	  devicerb += diskStatTBRead(threadContext->pos.diskStats);
	  devicewb += diskStatTBWrite(threadContext->pos.diskStats);
	  diskStatRestart(threadContext->pos.diskStats); // reset
	}
	
	if (twb + trb >= ignorefirst) {

	  const double gaptime = thistime - lastprintedtime;

	  double readB     = (trb - last_trb) / gaptime;
	  double readIOPS  = (tri - last_tri) / gaptime;
	
	  double writeB    = (twb - last_twb) / gaptime;
	  double writeIOPS = (twi - last_twi) / gaptime;
	
	  if ((tri - last_tri) || (twi - last_twi) || (devicerb - last_devicerb) || (devicewb - last_devicewb)) { // if any IOs in the period to display note the time
	    lastprintedtime = thistime;
	  }

	  const double elapsed = thistime - starttime;
	  fprintf(stderr,"[%2.2lf / %zd] read ", elapsed, threadContext->numThreads);
	  //fprintf(stderr,"%5.0lf", TOMB(readB));
	  commaPrint0dp(stderr, TOMB(readB));
	  fprintf(stderr," MB/s (");
	  //fprintf(stderr,"%7.0lf", readIOPS);
	  commaPrint0dp(stderr, readIOPS);
	  fprintf(stderr," IOPS / %.0lf), write ", (readIOPS == 0) ? 0 : (readB * 1.0) / (readIOPS));

	  //fprintf(stderr,"%5.0lf", TOMB(writeB));
	  commaPrint0dp(stderr, TOMB(writeB));
	  fprintf(stderr," MB/s (");
	  //fprintf(stderr,"%7.0lf", writeIOPS);
	  commaPrint0dp(stderr, writeIOPS);
	
	  fprintf(stderr," IOPS / %.0lf), total %.2lf GiB", (writeIOPS == 0) ? 0 : (writeB * 1.0) / (writeIOPS), TOGiB(trb + twb));
	  if (verbose >= 1) {
	    fprintf(stderr," == %zd %zd %zd %zd s=%.5lf == ", trb, tri, twb, twi, gaptime);
	  }
	  if (threadContext->pos.diskStats) {
	    fprintf(stderr," (R %.0lf MB/s, W %.0lf MB/s)", TOMB(devicerb / gaptime), TOMB(devicewb / gaptime));
	  }
	  fprintf(stderr,"\n");

	  if (fp) {
	    fprintf(fp, "%.4lf\t%lf\t%.1lf\t%.0lf\t%.1lf\t%.0lf\t%.1lf\t%.1lf\n", elapsed, thistime, TOMB(readB), readIOPS, TOMB(writeB), writeIOPS, TOMB(devicerb / gaptime), TOMB(devicewb / gaptime));
	    fflush(fp);
	  }
	
	  if (threadContext->exitIOPS && twb > threadContext->maxbdSize) {
	    if (writeIOPS + readIOPS < threadContext->exitIOPS) {
	      exitcount++;
	      if (exitcount >= 10) {
		fprintf(stderr,"*warning* early exit after %.1lf GB due to %zd periods of low IOPS (%.0lf < % zd)\n", TOGB(twb), exitcount, writeIOPS + readIOPS, threadContext->exitIOPS);
		keepRunning = 0;
		break;
	      }
	    }
	  }

	  last_trb = trb;
	  last_tri = tri;
	  last_twb = twb;
	  last_twi = twi;
	} // ignore first
	lasttime = thistime;
    }
    usleep(1000000*TIMEPERLINE/10); // display to 0.001 s
    
    if (thistime > starttime + threadContext->finishTime + 30) {
      fprintf(stderr,"*error* still running! watchdog termination (%.0lf > %.0lf + %zd\n", thistime, starttime, threadContext->finishTime + 30);
      keepRunning = 0;
      exit(-1);
    }
  }
  if (fp) {
    fclose(fp); fp = NULL;
  }

  //  fprintf(stderr,"start %.0lf last printed %.0lf this %.0lf actual %.0lf\n",0.0f, lastprintedtime - starttime, thistime - lastprintedtime, thistime - starttime);
  double tm = lastprintedtime - starttime;
  if (thistime - lastprintedtime >= 2) {
    tm = thistime - starttime; // if more than two seconds since a non zero line
  }
  fprintf(stderr,"*info* summary: read %.0lf MB/s (%.0lf IOPS), ", TOMB(total_printed_r_bytes)/tm, total_printed_r_iops/tm);
  fprintf(stderr,"write %.0lf MB/s (%.0lf IOPS)\n", TOMB(total_printed_w_bytes)/tm, total_printed_w_iops/tm);

  if (fpmysql) {
    fprintf(fpmysql, "CREATE TABLE if not exists benchmarks (id int not null auto_increment, date datetime not null, blockdevice char(30) not null, read_mbs float not null, read_iops float not null, write_mbs float not null, write_iops float not null, command char(100) not null, version char(10) not null, machine char(20) not null, iotype char(10) not null, opsize char(10) not null, iopattern char(10) not null, qd int not null, devicestate char(15) not null, threads int not null, read_total_gb float not null, write_total_gb float not null, tool char(10) not null, runtime float not null, mysqloptions char(200) not null, os char(20) not null, degraded int not null, k int not null, m int not null, checksum char(15), encryption char(10), cache int not null, unixdate int not null, machineUptime int, totalRAM bigint, precondition char(50), primary key(id));\n");
    fprintf(fpmysql, "insert into benchmarks set tool='spit', date=NOW(), unixdate='%.0lf', read_mbs='%.0lf', read_iops='%.0lf', write_mbs='%.0lf', write_iops='%.0lf'", floor(starttime), TOMB(total_printed_r_bytes)/tm, total_printed_r_iops/tm, TOMB(total_printed_w_bytes)/tm, total_printed_w_iops/tm);
    fprintf(fpmysql, ", read_total_gb='%.1lf', write_total_gb='%.1lf'", TOGB(trb), TOGB(twb));
    fprintf(fpmysql, ", threads='%zd', runtime='%.0lf'", threadContext->numThreads, ceil(thistime) - floor(starttime)); // insert a range that covers everything, including the printed values
    fprintf(fpmysql, ", mysqloptions='%s'", threadContext->mysqloptions);
    fprintf(fpmysql, ", command='%s'", threadContext->commandstring);
    fprintf(fpmysql, ", machineUptime='%zd'", getUptime());
    fprintf(fpmysql, ", totalRAM='%zd'", totalRAM());
    {
      char *os = getValue(threadContext->mysqloptions2, "os=");
      fprintf(fpmysql, ", os='%s'", os);
      if (os) free(os);
    }

    {
      char *version = getValue(threadContext->mysqloptions2, "version=");
      fprintf(fpmysql, ", version='%s'", version);
      if (version) free(version);
    }

    {
      char *value = getValue(threadContext->mysqloptions2, "machine=");
      if (strcmp(value,"NULL")==0) {
	value = hostname();
      }
      fprintf(fpmysql, ", machine='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions2, "blockdevice=");
      fprintf(fpmysql, ", blockdevice='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "iotype=");
      fprintf(fpmysql, ", iotype='%s'", value);
      if (value) free(value);
    }


    {
      char *value = getValue(threadContext->mysqloptions, "precondition=");
      fprintf(fpmysql, ", precondition='%s'", value);
      if (value) free(value);
    }


    {
      char *value = getValue(threadContext->mysqloptions, "opsize=");
      fprintf(fpmysql, ", opsize='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "iopattern=");
      fprintf(fpmysql, ", iopattern='%s'", value);
      if (value) free(value);
    }


    {
      char *value = getValue(threadContext->mysqloptions, "qd=");
      fprintf(fpmysql, ", qd='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "devicestate=");
      fprintf(fpmysql, ", devicestate='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "degraded=");
      fprintf(fpmysql, ", degraded='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "k=");
      fprintf(fpmysql, ", k='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "m=");
      fprintf(fpmysql, ", m='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "checksum=");
      fprintf(fpmysql, ", checksum='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "encryption=");
      fprintf(fpmysql, ", encryption='%s'", value);
      if (value) free(value);
    }

    {
      char *value = getValue(threadContext->mysqloptions, "cache=");
      fprintf(fpmysql, ", cache='%s'", value);
      if (value) free(value);
    }

    fprintf(fpmysql, ";\n");
    fprintf(fpmysql, "select \"last insert id\", last_insert_id();\n");
    fclose(fpmysql);
  }


  
  
  if (verbose) fprintf(stderr,"*info* finished thread timer\n");
  keepRunning = 0;
  return NULL;
}



void jobRunThreads(jobType *job, const int num, char *filePrefix,
		   const size_t minSizeInBytes,
		   const size_t maxSizeInBytes,
		   const size_t timetorun, const size_t dumpPos, char *benchmarkName, const size_t origqd,
		   unsigned short seed, const char *savePositions, diskStatType *d, const double timeperline, const double ignorefirst, const size_t verify,
		   char *mysqloptions, char *mysqloptions2, char *commandstring) {
  pthread_t *pt;
  CALLOC(pt, num+1, sizeof(pthread_t));

  threadInfoType *threadContext;
  CALLOC(threadContext, num+1, sizeof(threadInfoType));

  keepRunning = 1;

  if (seed == 0) {
    const double d = timedouble() * 100.0; // use the fractions so up/return is different
    const unsigned long d2 = (unsigned long)d;
    seed = d2 & 0xffff;
    if (verbose) fprintf(stderr,"*info* setting seed based on the time to %d\n", seed);
  }

  positionContainer **allThreadsPC;
  CALLOC(allThreadsPC, num, sizeof(positionContainer*));

  const double thetime = timedouble();
  const size_t UUID = thetime * 10;

    

  size_t *go = malloc(sizeof(size_t));
  *go = 0;
  
  for (int i = 0; i < num + 1; i++) { // +1 as the timer is the last onr
    
    threadContext[i].go = go;
    threadContext[i].filePrefix = filePrefix;
    threadContext[i].mysqloptions = mysqloptions;
    threadContext[i].mysqloptions2 = mysqloptions2;
    threadContext[i].commandstring = commandstring;
    threadContext[i].ignoreResults = 0;
    threadContext[i].id = i;
    threadContext[i].runTime = timetorun;
    threadContext[i].multipleTimes = 0;
    threadContext[i].finishTime = timetorun;
    threadContext[i].exitIOPS = 0;
    if (i == num) {
      threadContext[i].benchmarkName = benchmarkName;
    } else {
      threadContext[i].benchmarkName = NULL;
    }
    threadContext[i].UUID = UUID;
    positionContainerInit(&threadContext[i].pos, threadContext[i].UUID);
    allThreadsPC[i] = &threadContext[i].pos;
    threadContext[i].numThreads = num;
    threadContext[i].allPC = allThreadsPC;
  }

  for (int i = 0; i < num; i++) {
    size_t localminbdsize = minSizeInBytes, localmaxbdsize = maxSizeInBytes;
    threadContext[i].runTime = timetorun;
    threadContext[i].finishTime = timetorun;
    threadContext[i].exitIOPS = 0;
    threadContext[i].jumbleRun = 0;
    threadContext[i].minSizeInBytes = minSizeInBytes;
    threadContext[i].maxSizeInBytes = maxSizeInBytes;

    int seqFiles = 1;
    size_t seqFilesMaxSizeBytes = 0;
    size_t bs = 4096, highbs = 4096;

    lengthsInit(&threadContext[i].len);
    
    if (verbose >= 2) {
      fprintf(stderr,"*info* setting up thread %d\n", i);
    }
    char * charBS = strchr(job->strings[i], 'M');
    if (charBS && *(charBS+1)) {

      char *endp = NULL;
      bs = 1024L * 1024 * strtod(charBS+1, &endp);
      if (bs < 512) bs = 512;
      highbs = bs;
      if (*endp == '-') {
	int nextv = atoi(endp+1);
	if (nextv > 0) {
	  highbs = 1024L * 1024 * nextv;
	}
      }
    }
    if (highbs < bs) {
      highbs = bs;
    }


    
    size_t mod = 1;
    size_t remain = 0;
    { // specify the block device size in GiB
      char *charG = strchr(job->strings[i], 'G');
      if (charG && *(charG+1)) {
	double lowg = 0, highg = 0;

	if ((num > 1) && (*(charG+1) == '_')) {
	  // 2: 1/1
	  lowg = minSizeInBytes + (i * 1.0 / (num)) * (maxSizeInBytes - minSizeInBytes);
	  highg = minSizeInBytes + ((i+1) * 1.0 / (num)) * (maxSizeInBytes - minSizeInBytes);
	} else if ((num > 1) && (*(charG+1) == '%')) {
	  // 2: 1/1
	  mod = num;
	  remain = i;
	} else {
	  splitRange(charG + 1, &lowg, &highg);
	  if (lowg > highg) {
	    fprintf(stderr,"*error* low range needs to be lower [%.1lf, %.1lf]\n", lowg, highg);
	    lowg = 0;
	    //	  exit(1);
	  }
	  lowg = lowg * 1024 * 1024 * 1024;
	  highg = highg * 1024 * 1024 * 1024;
	}
	if (lowg == highg) { // if both the same, same as just having 1
	  lowg = minSizeInBytes;
	}
	if (lowg < minSizeInBytes) lowg = minSizeInBytes;
	if (lowg > maxSizeInBytes) lowg = minSizeInBytes;
	if (highg > maxSizeInBytes) highg = maxSizeInBytes;
	if (highg < minSizeInBytes) highg = minSizeInBytes;
	if (highg - lowg < 4096) { // if both the same, same as just having 1
	  lowg = minSizeInBytes;
	  highg = maxSizeInBytes;
	}
	localminbdsize = alignedNumber(lowg, 4096);
	localmaxbdsize = alignedNumber(highg, 4096);
      }
    }

    { // specify the block device size in bytes
      char *charG = strchr(job->strings[i], 'b');
      if (charG && *(charG+1)) {
	double lowg = 0, highg = 0;
	splitRange(charG + 1, &lowg, &highg);
	localminbdsize = lowg;
	localmaxbdsize = highg;
	if (localminbdsize == localmaxbdsize) { 
	  localminbdsize = 0;
	}
	if (minSizeInBytes > maxSizeInBytes) {
	  fprintf(stderr,"*error* low range needs to be lower [%.1lf, %.1lf]\n", lowg, highg);
	  exit(1);
	}
      }
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
	threadContext[num].exitIOPS = atoi(charI + 1);
	fprintf(stderr,"*info* exit IOPS set to %zd\n", threadContext[num].exitIOPS);
      }
    }
    

    {
      char *charI = strchr(job->strings[i], 'J');
      
      if (charI && *(charI + 1)) {
	int v = atoi(charI + 1);
	if (v < 1) {v = 1;}
	threadContext[i].jumbleRun = v;
	fprintf(stderr,"*info* jumbleRun set to %zd\n", threadContext[i].jumbleRun);
      }
    }
    



    // 'O' is really 'X1'
    size_t runXtimes = 0;
    {
      char *oOS = strchr(job->strings[i], 'O');
      if (oOS) {
	threadContext[i].runTime = -1;
	threadContext[i].finishTime = -1;
	threadContext[num].finishTime = -1; // the timer
	runXtimes = 1;
      }
    }

    char *multLimit = strchr(job->strings[i], 'x');
    if (multLimit && *(multLimit+1)) {
      char *endp = NULL;
      threadContext[i].multipleTimes = (size_t) (strtod(multLimit+1, &endp));
      threadContext[i].runXtimesTI = 1;
      threadContext[i].runTime = -1;
      threadContext[i].finishTime = -1;
      threadContext[num].finishTime = -1;
    }

    {
      char *charX = strchr(job->strings[i], 'X');
      
      if (charX && *(charX+1)) {
	runXtimes = atoi(charX + 1);
	threadContext[i].runTime = -1;
	threadContext[i].finishTime = -1;
	threadContext[num].finishTime = -1; // the timer
      }
    }
    threadContext[i].runXtimesTI = runXtimes;
    
    

    size_t actualfs = fileSizeFromName(job->devices[i]);
    threadContext[i].minbdSize = localminbdsize;
    threadContext[i].maxbdSize = localmaxbdsize;
    if (threadContext[i].filePrefix == NULL) {
      // if it should be there
      if (threadContext[i].maxbdSize > actualfs) {
	threadContext[i].maxbdSize = actualfs;
	fprintf(stderr,"*warning* file is smaller than specified file, shrinking to %zd\n", threadContext[i].maxbdSize);
      }
    }

        charBS = strchr(job->strings[i], 'k');
    if (charBS && *(charBS+1)) {

      char *endp = NULL;
      bs = 1024 * strtod(charBS+1, &endp);
      if (bs < 512) bs = 512;
      highbs = bs;
      if (*endp == '-' || *endp == ':') {
	int nextv = atoi(endp+1);
	if (nextv > 0) {
	  highbs = 1024 * nextv;
	}
      }
      if (*endp == '-') 
	lengthsSetupLowHighAlignSeq(&threadContext[i].len, bs, highbs, 4096);
      else
	lengthsSetupLowHighAlignPower(&threadContext[i].len, bs, highbs, 4096);	
    } else {
      lengthsAdd(&threadContext[i].len, bs, 1);
    }

    
    size_t mp = (size_t) ((threadContext[i].maxbdSize - threadContext[i].minbdSize) / bs);
    
    if (verbose) {
      fprintf(stderr,"*info* device '%s', size %.3lf GiB, minsize of %zd, maximum %zd positions\n", job->devices[i], TOGiB(threadContext[i].maxbdSize), bs, mp);
    }

    // use 1/4 of free RAM
    size_t useRAM = MIN((size_t)15L*1024*1024*1024, freeRAM() / 2); // 1L*1024*1024*1024;
    
    //    fprintf(stderr,"use ram %zd\n", useRAM);
    size_t fitinram = useRAM / num / sizeof(positionType);
    if ((verbose || (fitinram < mp)) && (i == 0)) {
      fprintf(stderr,"*info* using %.3lf GiB RAM for positions, we can store ", TOGiB(useRAM));
      commaPrint0dp(stderr, fitinram);
      fprintf(stderr," positions in RAM (%.1lf %%)\n", fitinram >= mp ? 100.0 : fitinram * 100.0/mp);
    }


    
    size_t countintime = mp;
    //        fprintf(stderr,"*info* runX %zd ttr %zd\n", threadContext[i].multipleTimes, threadContext[i].runTime);
    if ((long)threadContext[i].runTime > 0) { // only limit based on time if the time is positive

      int ESTIMATEIOPS=getIOPSestimate(job->devices[i], bs);
      
      countintime = threadContext[i].runTime * ESTIMATEIOPS;
      if ((verbose || (countintime < mp)) && (i == 0)) {
	fprintf(stderr,"*info* in %zd seconds, at %d a second, would have at most ", threadContext[i].runTime, ESTIMATEIOPS);
	commaPrint0dp(stderr, countintime);
	fprintf(stderr," positions (run %zd times)\n", threadContext[i].runXtimesTI);
      }
    } else {
      if (i==0) fprintf(stderr,"*info* skipping time based limits\n");
    }
    size_t sizeLimitCount = (size_t)-1;
    if (limit != (size_t)-1) {
      sizeLimitCount = limit / bs;
      if (i == 0) {
	fprintf(stderr,"*info* to limit sum of lengths to %.1lf GiB, with avg size of %zd, requires %zd positions\n", TOGiB(limit), bs, sizeLimitCount);
      }
    }

    size_t mp2 = MIN(sizeLimitCount, MIN(countintime, MIN(mp, fitinram)));
    if (mp2 != mp) {
      mp = mp2;
      if (verbose) {
	//      if (i == 0) {
	fprintf(stderr,"*info* device '%s', positions limited to %zd\n", job->devices[i], mp);
	//      }
      }
    }
      
    threadContext[i].jobstring = job->strings[i];
    threadContext[i].jobdevice = job->devices[i];
    threadContext[i].jobdeviceid = job->deviceid[i];
    threadContext[i].waitfor = 0;
    threadContext[i].exec = (job->delay[i] != 0);
    threadContext[i].prewait = job->delay[i];
    threadContext[i].blockSize = bs;
    threadContext[i].highBlockSize = highbs;
    threadContext[i].rerandomize = 0;
    threadContext[i].addBlockSize = 0;

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



    {
      char *sf = strchr(job->strings[i], 's');
      if (sf && *(sf+1)) {
	double low = 0, high = 0;
	int ret = splitRange(sf + 1, &low, &high);
	if (ret == 0) {
	} else if (ret == 1) {
	  seqFilesMaxSizeBytes = 0;
	} else {
	  seqFilesMaxSizeBytes = ceil(high) * 1024;
	}
	seqFiles = ceil(low);
      }
    }


    
    {
      char *sf = strchr(job->strings[i], '@');
      if (sf) {
	fprintf(stderr,"*warning* ignoring results for command '%s'\n", job->strings[i]);
	threadContext[i].ignoreResults = 1;
	threadContext[num].allPC[i] = NULL;
      }
    }


    {
      char *iR = strchr(job->strings[i], 'n');
      if (iR) {// && *(iR+1)) {
	threadContext[i].rerandomize = 1;
      }
    }

    {
      char *iR = strchr(job->strings[i], 'N');
      if (iR) {// && *(iR+1)) {
	threadContext[i].addBlockSize = 1;
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
	threadContext[i].finishTime = atoi(iTO+1);
      }
    }


    size_t qDepth = origqd, QDbarrier = 0;
   
    {
      char *qdd = strchr(job->strings[i], 'q');
      if (qdd && *(qdd+1)) {
	qDepth = atoi(qdd+1);
      }
    }
    {
      char *qdd = strchr(job->strings[i], 'Q');
      if (qdd && *(qdd+1)) {
	QDbarrier = 1;
	qDepth = atoi(qdd+1);
      }
    }

    threadContext[i].QDbarrier = QDbarrier;
    if (qDepth < 1) qDepth = 1;
    if (qDepth > 65535) qDepth = 65535;

    char *pChar = strchr(job->strings[i], 'P');
    {
      if (pChar && *(pChar+1)) {
	size_t newmp = atoi(pChar + 1);
	if (newmp < mp) {
	  mp = newmp;
	}
	if (mp < 1) mp = 1;
	if (mp <= qDepth) {
	  qDepth = mp;
	}
      }
    }

    size_t uniqueSeeds = 0, verifyUnique = 0;
    {
      // metaData mode is random, verify all writes and flushing, sets QD to 1
      char *iR = strchr(job->strings[i], 'u');
      if (iR) {
	metaData = 1;
	threadContext[i].flushEvery = 1;
	uniqueSeeds = 1;
	//	qDepth = 1;
      }
    }


    {
      // metaData mode is random, verify all writes and flushing, sets QD to 1
      char *iR = strchr(job->strings[i], 'U');
      if (iR) {
	metaData = 1;
	threadContext[i].flushEvery = 1;
	uniqueSeeds = 1;
	qDepth = 1;
	verifyUnique = 1;
      }
    }


    threadContext[i].queueDepth = qDepth;


    long startingBlock = -99999;
    if (strchr(job->strings[i], 'z')) {
      startingBlock = 0;
    }

    {
      char *ZChar = strchr(job->strings[i], 'Z');
      if (ZChar && *(ZChar+1)) {
	startingBlock = atoi(ZChar + 1);
	if (startingBlock < 0) {
	  startingBlock = 0;
	}
      }
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


    size_t speedMB = 0;
    {
      char *RChar = strchr(job->strings[i], 'S');
      if (RChar && *(RChar+1)) {
	speedMB = atoi(RChar + 1); // if specified
      }
    }
    threadContext[i].speedMB = speedMB;

    
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

    //    threadContext[i].pos.maxbdSize = threadContext[i].maxbdSize;
    

    positionContainerSetup(&threadContext[i].pos, mp);
    threadContext[i].seqFiles = seqFiles;
    threadContext[i].seqFilesMaxSizeBytes = seqFilesMaxSizeBytes;
    threadContext[i].rw = rw;
    threadContext[i].startingBlock = startingBlock;
    threadContext[i].mod = mod;
    threadContext[i].remain = remain;
    threadContext[i].metaData = metaData;
    threadContext[i].uniqueSeeds = uniqueSeeds;
    threadContext[i].verifyUnique = verifyUnique;
    threadContext[i].dumpPos = dumpPos;
    
  } // setup all threads

  size_t waitft = 0;
  for (int i = 0; i < num; i++) {
    if (threadContext[i].exec == 0) {
      waitft++;
    }
  }
  for (int i = 0; i < num + 1; i++) { // plus one as the last one [num] is the timer thread
    threadContext[i].waitForThreads = waitft;
  }



  
  // use the device and timing info from context[num]
  threadContext[num].pos.diskStats = d;
  threadContext[num].timeperline = timeperline;
  threadContext[num].ignorefirst = ignorefirst;
  threadContext[num].minbdSize = minSizeInBytes;
  threadContext[num].maxbdSize = maxSizeInBytes;
  // get disk stats before starting the other threads

  if (threadContext[num].pos.diskStats) {
    diskStatStart(threadContext[num].pos.diskStats);
  }

  
  pthread_mutex_t mutex;
  pthread_mutex_init(&mutex, NULL);
  for (int i = 0; i < num; i++) {
    threadContext[i].gomutex = &mutex;
  }
    
  
  pthread_create(&(pt[num]), NULL, runThreadTimer, &(threadContext[num]));
  for (int i = 0; i < num; i++) {
    pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]));
  }

  // wait for all threads
  for (int i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
    lengthsFree(&threadContext[i].len);
  }
  keepRunning = 0; // the 
  // now wait for the timer thread (probably don't need this)
  pthread_join(pt[num], NULL);


  if (savePositions || verify) {
    // print stats 
    for (int i = 0; i < num; i++) {
      positionLatencyStats(&threadContext[i].pos, i);
    }

    positionContainer *origpc;
    CALLOC(origpc, num, sizeof(positionContainer));
    for (int i = 0; i < num; i++) {
      origpc[i] = threadContext[i].pos;
    }
    
    positionContainer mergedpc = positionContainerMerge(origpc, num);

    if (savePositions) {
      fprintf(stderr, "*info* saving positions to '%s' ... ", savePositions);  fflush(stderr);
      positionContainerSave(&mergedpc, savePositions, mergedpc.maxbdSize, 0, job);
      fprintf(stderr, "finished\n"); fflush(stderr);
    }
    
    // histogram
    histogramType histRead, histWrite;
    histSetup(&histRead, 0, 10, 1e-6);
    histSetup(&histWrite, 0, 10, 1e-6);
    
    for (int i = 0; i < (int) mergedpc.sz; i++) {
      if (mergedpc.positions[i].action == 'R') 
	histAdd(&histRead, mergedpc.positions[i].finishTime - mergedpc.positions[i].submitTime);
      else if (mergedpc.positions[i].action == 'W') 
	histAdd(&histWrite, mergedpc.positions[i].finishTime - mergedpc.positions[i].submitTime);
    }
    double median, three9, four9, five9;
    if (histCount(&histRead)) {
      histSumPercentages(&histRead, &median, &three9, &four9, &five9, 1000);
      fprintf(stderr,"*info* read latency:  mean = %.3lf ms, median = %.2lf ms, 99.9%% <= %.2lf ms, 99.99%% <= %.2lf ms, 99.999%% <= %.2lf ms\n", 1000 * histMean(&histRead), median, three9, four9, five9);
      histSave(&histRead, "spit-latency-read.txt", 1000);

      FILE *fp = fopen("spit-latency-read.gnu", "wt");
      if (fp) {
	fprintf(fp, "set key above\n");
	fprintf(fp, "set title 'Response Time Histogram - Confidence Level Plot\n");
	fprintf(fp, "set log y\n");
	fprintf(fp, "set xtics auto\n");
	fprintf(fp, "set y2tics 10\n");
	fprintf(fp, "set grid\n");
	fprintf(fp, "set xrange [0:%lf]\n", five9 * 1.1);
	fprintf(fp, "set y2range [0:100]\n");
	fprintf(fp, "set xlabel 'Time (ms)'\n");
	fprintf(fp, "set ylabel 'Count'\n");
	fprintf(fp, "set y2label 'Confidence level'\n");
	fprintf(fp, "plot 'spit-latency-read.txt' using 1:2 with imp title 'Read Latency', 'spit-latency-read.txt' using 1:3 with lines title '%% Confidence' axes x1y2,'<echo %lf 100000' with imp title 'ART=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.9%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.99%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.999%%=%.2lf' axes x1y2\n", median, median, three9, three9, four9, four9, five9, five9);
      } else {
	perror("filename");
      }
      if (fp) {
	fclose(fp); fp = NULL;
      }
      
    }
    if (histCount(&histWrite)) {
      histSumPercentages(&histWrite, &median, &three9, &four9, &five9, 1000);
      fprintf(stderr,"*info* write latency: mean = %.3lf ms, median = %.2lf ms, 99.9%% <= %.2lf ms, 99.99%% <= %.2lf ms, 99.999%% <= %.2lf ms\n", 1000.0 * histMean(&histWrite), median, three9, four9, five9);
      histSave(&histWrite, "spit-latency-write.txt", 1000);

      FILE *fp = fopen("spit-latency-write.gnu", "wt");
      if (fp) {
	fprintf(fp, "set key above\n");
	fprintf(fp, "set title 'Response Time Histogram - Confidence Level Plot\n");
	fprintf(fp, "set log y\n");
	fprintf(fp, "set xtics auto\n");
	fprintf(fp, "set y2tics 10\n");
	fprintf(fp, "set grid\n");
	fprintf(fp, "set xrange [0:%lf]\n", five9 * 1.1);
	fprintf(fp, "set y2range [0:100]\n");
	fprintf(fp, "set xlabel 'Time (ms)'\n");
	fprintf(fp, "set ylabel 'Count'\n");
	fprintf(fp, "set y2label 'Confidence Level'\n");
	fprintf(fp, "plot 'spit-latency-write.txt' using 1:2 with imp title 'Write Latency', 'spit-latency-write.txt' using 1:3 with lines title '%% Confidence' axes x1y2,'<echo %lf 100000' with imp title 'ART=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.9%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.99%%=%.2lf' axes x1y2, '<echo %lf 100000' with imp title '99.999%%=%.2lf' axes x1y2\n", median, median, three9, three9, four9, four9, five9, five9);
      } else {
	perror("filename");
      }
      if (fp) {
	fclose(fp); fp = NULL;
      }
      
    }
    histFree(&histRead);
    histFree(&histWrite);
    
    if (verify) {
      positionContainerCheckOverlap(&mergedpc);
      size_t direct = O_DIRECT;
      for (int i = 0; i < num; i++) { // check all threads, if any aren't direct
	if (threadContext[i].o_direct == 0) {
	  direct = 0;
	}
      }
      int errors = verifyPositions(&mergedpc, 256, job, direct); 
      if (errors) {
	exit(1);
      }
    }
    
    positionContainerFree(&mergedpc);
    free(origpc);
  }

  // free
  for (int i = 0; i < num; i++) {
    positionContainerFree(&threadContext[i].pos);
    free(threadContext[i].randomBuffer);
  }
  
  

  free(go);
  free(allThreadsPC);
  free(threadContext);
  free(pt);
}


void jobAddDeviceToAll(jobType *job, const char *device) {
  for (int i = 0; i < job->count; i++) {
    job->devices[i] = strdup(device);
  }
}

size_t jobCount(jobType *job) {
  return job->count;
}

size_t jobRunPreconditions(jobType *preconditions, const size_t count, const size_t minSizeBytes, const size_t maxSizeBytes) {
  if (count) {
    size_t gSize = alignedNumber(maxSizeBytes, 4096);
    size_t coverage = 1;
    size_t jumble = 0;
    
    for (int i = 0; i < (int) count; i++) {
      fprintf(stderr,"*info* precondition %d: device '%s', command '%s'\n", i+1, preconditions->devices[i], preconditions->strings[i]);
      {
	char *charG = strchr(preconditions->strings[i], 'G');
	if (charG && *(charG+1)) {
	  // a G num is specified
	  gSize = alignedNumber(1024L * atof(charG + 1) * 1024 * 1024, 4096);
	}
      }


      double blockSize = 4;
      {
	char *charG = strchr(preconditions->strings[i], 'k');
	if (charG && *(charG+1)) {
	  // a k value is specified
	  blockSize = atof(charG + 1);
	}
      }

      size_t seqFiles = 0;
      {// seq or random
	char *charG = strchr(preconditions->strings[i], 's');
	if (charG && *(charG+1)) {
	  seqFiles = atoi(charG + 1);
	}
      }
      if (seqFiles != 0) {
	jumble = 1;
      }
      
      size_t exitIOPS = 25000; // 100MB/s lowerbound
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
	fprintf(stderr,"*info* %zd / %zd = X%zd coverage\n", gSize, maxSizeBytes, coverage);
      }
      
      char s[100];
      sprintf(s, "w k%g z s%zd J%zd b%zd X%zd nN I%zd q512", blockSize, seqFiles, jumble, maxSizeBytes, coverage, exitIOPS);
      free(preconditions->strings[i]);
      preconditions->strings[i] = strdup(s);
    }
    jobRunThreads(preconditions, count, NULL, minSizeBytes, maxSizeBytes, -1, 0, NULL, 128, 0 /*seed*/, 0 /*save positions*/, NULL, 1, 0, 0 /*noverify*/, NULL, NULL, NULL); 
    fprintf(stderr,"*info* preconditioning complete\n"); fflush(stderr);
  }
  return 0;
}
    
