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
int openmode = 0;

typedef struct {
  size_t threadid;
  size_t totalFileSpace;
  size_t maxthreads;
  size_t writesize;
  size_t filesize;
  size_t numfiles;
  size_t *fileid;
  char *actions;
} threadInfoType;


size_t diskSpace() {
  struct statvfs buf;

  statvfs(".", &buf);
  size_t t =  buf.f_blocks;
  t = t * buf.f_bsize;
  return t;
}
    

int createAction(const char *filename, char *buf, const size_t size, const size_t writesize) {
  int fd = open(filename, openmode | O_CREAT | O_TRUNC | O_WRONLY, S_IRUSR | S_IWUSR);
  
  if (fd > 0) {
    size_t towrite = size;
    while (towrite > 0) {
      int written = write(fd, buf, MIN(towrite, writesize));
      //      fprintf(stderr,"%s %zd\n", filename, MIN(towrite, writesize));
      if (written > 0) {
	towrite -= written;
      } else if (written == 0) {
	fprintf(stderr,"zero byte write\n");
      } else {
	perror("write");
	break;
      }
    }
    fdatasync(fd);
    close(fd);
    if (verbose) fprintf(stderr,"*info* wrote file %s, size %zd\n", filename, size);
    return 0;
  } else {
    fprintf(stderr,"*error* didn't write %s, skipping\n", filename);
    return 1;
  }
}



int readAction(const char *filename, char *buf, size_t size) {
  int fd = open(filename, openmode | O_RDONLY);
  int ret = -1;
  
  if (fd > 0) {
    ret = 0;
    size_t toread = 0;
    size_t zerocount = 0;
    while (toread != size) {
      int readd = read(fd, buf, size);
      if (readd > 0) {
	toread += readd;
      } else if (readd == 0) {
	zerocount++;
	fprintf(stderr,"'%s' truncated at %zd before size %zd\n", filename, toread, size);
	break;
      } else {
	ret = -2;
	perror("read");
	break;
      }
    }
    close(fd);
    if (verbose) fprintf(stderr,"*info* read file %s, size %zd\n", filename, size);
  } else {
    ret = -3;
    //    fprintf(stderr,"*error* couldn't open %s, skipping\n", filename);
    //exit(1);
  }
  return ret;
}

#define MAXFILESIZE (1024*1024*10)

void* worker(void *arg) 
{
  threadInfoType *threadContext = (threadInfoType*)arg;
  const size_t totalfilespace = threadContext->totalFileSpace;
  size_t numfiles = threadContext->numfiles;
  char *buf = aligned_alloc(4096, MAXFILESIZE);
  memset(buf, 'z', MAXFILESIZE);

  char *s = aligned_alloc(4096, 1024*1024);

  size_t processed = 0, sum = 0;
  double starttime = timedouble(), lasttime = starttime;
  size_t lastfin = 0;
  
  char outstring[1000];
  //size_t actionSize = 100;
  //workQueueActionType *actionArray = calloc(actionSize, sizeof(workQueueActionType));

  while (!finished) {
    for (size_t i = 0; i < numfiles; i++) {

      if (finished) break;

      if ((i % threadContext->maxthreads) == threadContext->threadid) {
	size_t val = threadContext->fileid[i];
	
	
	if (threadContext->threadid == 0) {
	  const double thistime = timedouble();
	  const double tm = thistime - lasttime;
	    
	  if (tm > 1) {
	    const size_t wqfin = processed;
	    const size_t fin = wqfin - lastfin;
	    lastfin = wqfin;
	      
	    const double thistime = timedouble();
	    const double tm = thistime - lasttime;
	    lasttime = thistime;
	      
	    sprintf(outstring, "*info* [thread %zd/%zd] [files %zd (%zd) / %zd], finished %zd, %.0lf files/second, %.1lf GB, %.2lf LBA, %.0lf MB/s, %.1lf seconds (%.1lf)\n", threadContext->threadid+1, threadContext->maxthreads, wqfin, wqfin % threadContext->numfiles, threadContext->numfiles, processed, (fin/tm), TOGB(sum), sum * 1.0/totalfilespace, TOMB(sum * 1.0)/(thistime-starttime), thistime - starttime, tm);
	    lasttime = thistime;

	    if (bfp) {
	      fprintf(bfp, "%s", outstring);
	      //	  fflush(bfp);
	    }
	    fprintf(stderr,"%s", outstring);
	  }
	}
	// do the mahi
	  
	sprintf(s,"%02x/%02x/%010zd.thread%zd", (((unsigned int)val)/DEPTH) % DEPTH, ((unsigned int)val)%DEPTH, val, threadContext->threadid);
	//	  fprintf(stderr,"thread %zd, id %zd\n", threadContext->threadid, val);
	  
	//      fprintf(stderr,"%s", s);
	//      	  	  	  fprintf(stderr,"[%zd] %c %s\n", action.id, action.type, action.payload);
	switch (threadContext->actions[i]) {
	case 'W': 
	  if (createAction(s, buf, threadContext->filesize, threadContext->writesize) == 0) {
	    processed++;
	    sum += (threadContext->maxthreads * threadContext->filesize);
	  }
	  break;
	case 'R':
	  if (readAction(s, buf, threadContext->filesize) == 0) {
	    processed++;
	    sum += (threadContext->maxthreads * threadContext->filesize);
	  }
	  break;
	default:
	  abort();
	}	
      }
    }
  }
  
  free(buf);
  return NULL;
}


int threads = 32;
size_t filesize = 360*1024;
size_t writesize = 1024*1024;

void usage() {
  fprintf(stderr,"Usage: (run from a mounted folder) fsfiller [-T threads (%d)] [-k fileSizeKIB (%zd..%d)] [-K blocksizeKiB (%zd)] [-V(verbose)] [-r(read)] [-w(write)] [-B benchmark.out] [-R seed] [-u(unique filenames)] [-U(nonunique/with replacement]\n", threads, filesize/1024, MAXFILESIZE/1024, writesize/1024);
}

int main(int argc, char *argv[]) {


  int read = 0;
  int opt = 0, help = 0;
  unsigned int seed = timedouble() * 10;
  seed = seed % 65536;
  size_t unique = 1;
  
  while ((opt = getopt(argc, argv, "T:Vk:K:hrwB:R:uUsDd")) != -1) {
    switch (opt) {
    case 'B':
      benchmarkFile = optarg;
      break;
    case 'D':
      openmode = 0;
      break;
    case 'd':
      openmode = O_DIRECT;
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
  fprintf(stderr,"*info* number of threads %d, file size %zd (block size %zd), numFiles %zd, unique %zd, seed %u, O_DIRECT %d, read %d\n", threads, filesize, writesize, numFiles, unique, seed, openmode, read);

  size_t *fileid = calloc(numFiles, sizeof(size_t));
  char *actions = calloc(numFiles, sizeof(char));
  if (read) {
    memset(actions, 'R', numFiles);
  } else {
    memset(actions, 'W', numFiles);
  }
    
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
  
  

  size_t queueditems = 100000;
  workQueueInit(&wq, queueditems);

  finished = 0;


  
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


  // start threads

  // setup worker threads
  pthread_t *tid = calloc(threads, sizeof(pthread_t));
  
  threadInfoType *threadinfo = calloc(threads, sizeof(threadInfoType));
  
  for (int i = 0; i < threads; i++){
    threadinfo[i].threadid = i;
    threadinfo[i].maxthreads = threads;
    threadinfo[i].writesize = writesize;
    threadinfo[i].filesize = filesize;
    threadinfo[i].totalFileSpace = totalfilespace;
    threadinfo[i].numfiles = numFiles;
    threadinfo[i].actions = actions;
    threadinfo[i].fileid = fileid;
    int error = pthread_create(&(tid[i]), NULL, &worker, &threadinfo[i]);
    if (error != 0) {
      printf("\nThread can't be created :[%s]", strerror(error)); 
    }
  }

  while (!finished) {
    sleep(1);
  }

  free(threadinfo);
  free(tid);
  free(fileid);

  workQueueFree(&wq);
  //  fclose(bfp);
}
