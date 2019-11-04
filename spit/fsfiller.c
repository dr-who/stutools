#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L
#define _XOPEN_SOURCE 500

#include "workQueue.h"
#include "utils.h"


#include <stdlib.h>
#include <malloc.h>
#include <assert.h>
#include <unistd.h>

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <sys/statvfs.h>


#include <pthread.h>
#include <string.h>

#define DEPTH 100

workQueueType wq;
size_t finished;
int verbose = 0;
int keepRunning = 1;
char *benchmarkFile = NULL;
FILE *bfp = NULL;

typedef struct {
  size_t threadid;
  size_t totalFileSpace;
} threadInfoType;


size_t diskSpace() {
  struct statvfs buf;

  statvfs(".", &buf);
  size_t t =  buf.f_blocks;
  t = t * buf.f_bsize;
  return t;
}
    

void createAction(const char *filename, char *buf, const size_t size, const size_t writesize) {
  int fd = open(filename, O_DIRECT | O_CREAT | O_TRUNC | O_WRONLY, S_IRUSR | S_IWUSR);
  
  if (fd > 0) {
    size_t towrite = size;
    while (towrite > 0) {
      int written = write(fd, buf, MIN(towrite, writesize));
      //      fprintf(stderr,"%s %zd\n", filename, MIN(towrite, writesize));
      if (written >= 0) {
	towrite -= written;
      } else {
	perror("write");
      }
    }
    close(fd);
    if (verbose) fprintf(stderr,"*info* wrote file %s, size %zd\n", filename, size);
  } else {
    fprintf(stderr,"*error* didn't write %s, skipping\n", filename);
    //    exit(1);
  }
}



void readAction(const char *filename, char *buf, size_t size) {
  int fd = open(filename, O_DIRECT | O_RDONLY);
  
  if (fd > 0) {
    size_t toread = 0;
    while (toread != size) {
      int readd = read(fd, buf, size);
      if (readd >= 0) {
	toread += readd;
      } else {
	perror("read");
      }
    }
    close(fd);
    if (verbose) fprintf(stderr,"*info* read file %s, size %zd\n", filename, size);
  } else {
    fprintf(stderr,"*error* didn't read %s, skipping\n", filename);
    //exit(1);
  }
}


void* worker(void *arg) 
{
  threadInfoType *threadContext = (threadInfoType*)arg;
  const size_t totalfilespace = threadContext->totalFileSpace;
  char *buf = aligned_alloc(4096, 1024*1024*1);
  memset(buf, 'z', 1024*1024*1);

  char outstring[1000];
  
  while (1) {
    
    workQueueActionType *action = workQueuePop(&wq);
    
    if (action) {
      
      const size_t fin = workQueueFinished(&wq);
      if (fin % 1000 == 0) {
	const double tm = timedouble() - wq.startTime;
	const size_t sum = workQueueFinishedSize(&wq);
	
	sprintf(outstring, "*info* [thread %zd] [action %zd, '%s'], finished %zd, %.0lf files/second, %.1lf GB, LBAx %.2lf, %.0lf MB/s, %.1lf seconds\n", threadContext->threadid, action->id, action->payload, fin, fin/tm, TOGB(sum), sum * 1.0/totalfilespace, TOMB(sum)/tm, tm);
	if (bfp) {
	  fprintf(bfp, "%s", outstring);
	  //	  fflush(bfp);
	}
	fprintf(stderr,"%s", outstring);
      }
      
      switch(action->type) {
      case 'W': 
	//	fprintf(stderr,"[%zd] %c %s\n", action->id, action->type, action->payload);
	if (action->size < 1024*1024*1) {
	  createAction(action->payload, buf, action->size, 65536);
	} else {
	  fprintf(stderr,"*warning* big file ignored\n");
	}
	break;
      case 'R': 
	if (action->size < 1024*1024*1) {
	  readAction(action->payload, buf, action->size);
	} else {
	  fprintf(stderr,"*warning* big file ignored\n");
	}
	break;
      }
      
      free(action->payload);
      free(action);
    } else {
      //            fprintf(stderr,"starving\n");
    }
  }
  free(buf);
  return NULL;
}


int threads = 32;
size_t filesize = 360*1024;
size_t writesize = 1024*1024;

void usage() {
  fprintf(stderr,"Usage: (run from a mounted folder) fsfiller [-T threads (%d)] [-k fileSizeKIB (%zd)] [-K blocksizeKiB (%zd)] [-V(verbose)] [-r(read)] [-w(write)] [-B benchmark.out] [-R seed] [-u(unique filenames)] [-U(nonunique/with replacement]\n", threads, filesize/1024, writesize/1024);
}

int main(int argc, char *argv[]) {


  int read = 0;
  int opt = 0, help = 0;
  unsigned int seed = timedouble() * 10;
  seed = seed % 65536;
  size_t unique = 1;
  
  while ((opt = getopt(argc, argv, "T:Vk:K:hrwB:R:uUs")) != -1) {
    switch (opt) {
    case 'B':
      benchmarkFile = optarg;
      break;
    case 'r':
      read = 1;
      break;
    case 'R':
      seed = atoi(optarg);
      break;
    case 'w':
      read = 0;
      break;
    case 'h':
      help = 1;
      break;
    case 'k':
      filesize = 1024 * atoi(optarg);
      break;
    case 'K':
      writesize = 1024 * atoi(optarg);
      break;
    case 'T':
      threads = atoi(optarg);
      break;
    case 'u':
      unique = 1;
      break;
    case 'U':
      unique = 0; // with replacement
      break;
    case 's':
      unique = 2; // 2 is sequential
      break;
    case 'V':
      verbose = 1;
      break;
    }
  }

  if (help) {
    usage();
    exit(0);
  }
      
  size_t totalfilespace = diskSpace();
  size_t numFiles = (totalfilespace / filesize) * 0.95;

  srand(seed);
  fprintf(stderr,"*info* number of threads %d, file size %zd (block size %zd), numFiles %zd, unique %zd, seed %u\n", threads, filesize, writesize, numFiles, unique, seed);

  size_t *fileid = malloc(numFiles * sizeof(size_t));
  for (size_t i = 0; i < numFiles; i++) {
    if (unique) { // unique 1 or 2 for seq
      fileid[i] = i;
    } else {
      fileid[i] = rand() % numFiles;
    }
  }
  if (unique == 1) { // if randomise 
    for (size_t i = 0; i < numFiles; i++) {
      const size_t j = rand() % numFiles;
      size_t tmp = fileid[i];
      fileid[i] = fileid[j];
      fileid[j] = tmp;
    }
  }
  
  

  size_t queueditems = 10000;
  workQueueInit(&wq, queueditems);

  finished = 0;


  // setup worker threads
  pthread_t *tid = malloc(threads * sizeof(pthread_t));
  
  threadInfoType *threadinfo = malloc(threads * sizeof(threadInfoType));
  
  for (size_t i = 0; i < threads; i++){
    threadinfo[i].threadid = i;
    threadinfo[i].totalFileSpace = totalfilespace;
    int error = pthread_create(&(tid[i]), NULL, &worker, &threadinfo[i]);
    if (error != 0) {
      printf("\nThread can't be created :[%s]", strerror(error)); 
    }
  }


  
  char s[1000];
  fprintf(stderr,"*info* making %d top level directories\n", DEPTH);
  for (unsigned int id = 0; id < DEPTH; id++) {
    sprintf(s,"%02x", id);
    mkdir(s, 0777);
  }
  fprintf(stderr,"*info* making %d x %d two level directories\n", DEPTH, DEPTH);
  for (unsigned int id = 0; id < DEPTH * DEPTH; id++) {
    sprintf(s,"%02x/%02x", (((unsigned int)id)/DEPTH) % DEPTH, ((unsigned int)id)%DEPTH);
    mkdir(s, 0777);
  }

  if (benchmarkFile) {
    fprintf(stderr,"*info* opening benchmark file '%s'\n", benchmarkFile);
    bfp = fopen(benchmarkFile, "wt");
    if (!bfp) {
      perror("fsfiller");exit(1);
    }
  }
 
  size_t id = 0;
  while (!finished) {
    workQueueActionType *action = NULL;
    
    if (workQueueNum(&wq) < queueditems - 10) { // queue up to 10,000 items at a time
      if (!action) {
	action = malloc(sizeof(workQueueActionType));
	if (read) {
	  action->type = 'R';
	} else {
	  action->type = 'W';
	}
	size_t val = fileid[id % numFiles];
	sprintf(s,"%02x/%02x/%010zd", (((unsigned int)val)/DEPTH) % DEPTH, ((unsigned int)val)%DEPTH, val);
	action->payload = strdup(s);
	action->id = id++;
	action->size = filesize;
      }

      if (workQueuePush(&wq, action) != 0) {
	action = NULL;// it wasn't added
	fprintf(stderr,"wasn't added\n");
      }
    } else {
      //      fprintf(stderr,"sleeping...%zd in the queue\n", workQueueNum(&wq));
      usleep(100000);
      action = NULL;
    }
  }

  fclose(bfp);

  workQueueFree(&wq);
}
