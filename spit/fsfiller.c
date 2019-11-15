#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L
#define _XOPEN_SOURCE 500

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

size_t finished;
int verbose = 0;
int keepRunning = 1;
char *benchmarkFile = NULL;
FILE *bfp = NULL;
int openmode = 0;
char *dirPrefix = NULL;
size_t verify = 0, dofdatasync = 1;

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


size_t diskSpace(const char *prefix) {
  struct statvfs buf;

  statvfs(prefix, &buf);
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
    if (dofdatasync) fdatasync(fd);
    close(fd);
    if (verbose) fprintf(stderr,"*info* wrote file %s, size %zd, char %d\n", filename, size, buf[0]);
    return 0;
  } else {
    perror(filename);
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
	ret = -3;
	break;
      } else {
	ret = -2;
	perror("read");
	break;
      }
    }
    close(fd);
    if (verbose) fprintf(stderr,"*info* read file %s, size %zd, char %d\n", filename, size, buf[0]);
  } else {
    ret = -3;
    //        fprintf(stderr,"*error* couldn't open %s, skipping\n", filename);
    //exit(1);
  }
  //  fprintf(stderr,"ret %d\n", ret);
  return ret;
}

#define MAXFILESIZE (1024*1024*10)

void* worker(void *arg) 
{
  threadInfoType *threadContext = (threadInfoType*)arg;
  const size_t totalfilespace = threadContext->totalFileSpace;
  size_t numfiles = threadContext->numfiles;
  char *buf = aligned_alloc(4096, MAXFILESIZE); assert(buf);
  memset(buf, threadContext->threadid, MAXFILESIZE);

  char *verifybuf = aligned_alloc(4096, MAXFILESIZE); assert(verifybuf);
  

  char *s = aligned_alloc(4096, 1024*1024); assert(s);

  size_t processed = 0, sum = 0, skipped = 0;
  double starttime = timedouble(), lasttime = starttime;
  size_t lastfin = 0, pass = 0;
  
  char outstring[1000];

  while (!finished) {
    for (size_t i = 0; !finished && i < numfiles; i++) {

      if (i == 0) {
	pass++;
      }
      const size_t wqfin = processed;

	  
      {
	if (threadContext->threadid == 0) {
	  const double thistime = timedouble();
	  const double tm = thistime - lasttime;

	    
	  if (tm > 1) {
	    const size_t fin = wqfin - lastfin;
	    lastfin = wqfin;
	      
	    const double thistime = timedouble();
	    const double tm = thistime - lasttime;
	    lasttime = thistime;
	      
	    sprintf(outstring, "*info* [%zd] [pass %zd] [fileid %zd (%zd) / %zd], files %zd, %.0lf files/second, %.1lf GB, %.2lf LBA, %.0lf MB/s, %.1lf seconds (%.1lf), skipped %zd\n", threadContext->maxthreads, pass, wqfin, wqfin % threadContext->numfiles, threadContext->numfiles, processed * threadContext->maxthreads, (fin * threadContext->maxthreads/ tm), TOGB(sum), sum * 1.0/totalfilespace, TOMB(sum * 1.0)/(thistime-starttime), thistime - starttime, tm, skipped);
	    lasttime = thistime;

	    if (bfp) {
	      fprintf(bfp, "%s", outstring);
	      fflush(bfp);
	    }
	    fprintf(stderr,"%s", outstring);
	  }
	}
	// do the mahi
	
	unsigned int thisseed = threadContext->fileid[i] + threadContext->threadid;
	size_t val = rand_r(&thisseed);
	//	const size_t val = threadContext->fileid[i] + (i * threadContext->threadid);
	if (dirPrefix) {
	  sprintf(s, "%s/%02x/%02x/%010zd.thread.%zd", dirPrefix, (((unsigned int)val)/DEPTH) % DEPTH, ((unsigned int)val)%DEPTH, val, threadContext->threadid);
	} else {
	  sprintf(s, "%02x/%02x/%010zd.thread.%zd", (((unsigned int)val)/DEPTH) % DEPTH, ((unsigned int)val)%DEPTH, val, threadContext->threadid);
	}
	//	fprintf(stderr,"thread %zd, id %zd, filename '%s', dirPrefix '%s'\n", threadContext->threadid, val, s, dirPrefix ? dirPrefix : "");
	  
	//      	  	  	  fprintf(stderr,"[%zd] %c %s\n", action.id, action.type, action.payload);
	switch (threadContext->actions[i]) {
	case 'W': 
	  if (createAction(s, buf, threadContext->filesize, threadContext->writesize) == 0) {
	    processed++;
	    sum += (threadContext->maxthreads * threadContext->filesize);

	    if (verify) {
	      memset(verifybuf, 0xff, threadContext->filesize);
	      int ret = readAction(s, verifybuf, threadContext->filesize);
	      if (ret == 0) {
		if (memcmp(buf, verifybuf, threadContext->filesize) != 0) {
		  fprintf(stderr,"*error* verification error on %s", s);
		  exit(1);
		} else {
		  if (verbose) fprintf(stderr,"*info* verified file %s OK, char %d\n", s, verifybuf[0]);
		}
	      } else {
		perror(s);
		exit(1);
	      }
	    }
	    
	  }
	  break;
	case 'R': {}
	  int ret = readAction(s, buf, threadContext->filesize);
	  if (ret == 0) {
	    processed++;
	    //	      fprintf(stderr,"%s\n", s);
	    //	    fprintf(stderr,"process %zd ret %d\n", processed, ret);
	    sum += (threadContext->maxthreads * threadContext->filesize);
	  } else {
	    //	    fprintf(stderr,"processed %zd, skipped %s\n", wqfin, s);
	    skipped++;
	  }
	  break;
	default:
	  abort();
	}
      }
    }
  }
  free(s);
  free(verifybuf);
  free(buf);
  return NULL;
}


int threads = 32;
size_t filesize = 360*1024;
size_t writesize = 1024*1024;

void usage() {
  fprintf(stderr,"Usage: fsfiller [-F dirPrefix (.)] [-T threads (%d)] [-k fileSizeKIB (%zd..%d)] [-K blocksizeKiB (%zd)] [-V(verbose)] [-r(read)] [-w(write)] [-B benchmark.out] [-R seed (42)] [-u(unique filenames)] [-U(nonunique/with replacement] [-v (verify writes)]\n", threads, filesize/1024, MAXFILESIZE/1024, writesize/1024);
}

int main(int argc, char *argv[]) {


  int read = 0;
  int opt = 0, help = 0;
  unsigned int seed = 42;
  size_t unique = 1;
  size_t timelimit = 0;
  
  while ((opt = getopt(argc, argv, "T:Vk:K:hrwB:R:uUsDdt:F:vS")) != -1) {
    switch (opt) {
    case 'B':
      benchmarkFile = optarg;
      break;
    case 'D':
      openmode = 0;
      break;
    case 'F':
      dirPrefix = optarg;
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
    case 'S':
      dofdatasync = 0;
      break;
    case 't':
      timelimit = atoi(optarg);
      break;
    case 'v':
      verify = 1;
      break;
    case 'V':
      verbose++;
      break;
    }
    
  }

  if (help) {
    usage();
    exit(0);
  }
      
  size_t totalfilespace;
  if (dirPrefix) totalfilespace = diskSpace(dirPrefix);
  else totalfilespace = diskSpace(".");
  
  size_t numFiles = (totalfilespace / filesize) * 0.93 / threads;


  srand(seed);
  fprintf(stderr,"*info* dirPrefix '%s', diskspace %.0lf GB, number of threads %d, file size %zd (block size %zd), numFiles %zd, unique %zd, seed %u, O_DIRECT %d, read %d, %zd secs, verify %zd, fdatasync %zd\n", dirPrefix ? dirPrefix : ".", TOGB(totalfilespace), threads, filesize, writesize, numFiles * threads, unique, seed, openmode, read, timelimit, verify, dofdatasync);

  size_t *fileid = calloc(numFiles, sizeof(size_t)); assert(fileid);
  char *actions = calloc(numFiles, sizeof(char)); assert(actions);

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

  finished = 0;
  
  char s[1000];
  fprintf(stderr,"*info* making %d top level directories\n", DEPTH);
  for (unsigned int id = 0; id < DEPTH; id++) {
    if (dirPrefix) {
      sprintf(s,"%s/%02x", dirPrefix, id);
    } else {
      sprintf(s,"%02x", id);
    }
    mkdir(s, 0777);
  }
  fprintf(stderr,"*info* making %d x %d two level directories\n", DEPTH, DEPTH);
  for (unsigned int id = 0; id < DEPTH * DEPTH; id++) {
    if (dirPrefix) {
      sprintf(s,"%s/%02x/%02x", dirPrefix, (((unsigned int)id)/DEPTH) % DEPTH, ((unsigned int)id)%DEPTH);
    } else {
      sprintf(s,"%02x/%02x", (((unsigned int)id)/DEPTH) % DEPTH, ((unsigned int)id)%DEPTH);
    }
    mkdir(s, 0777);
  }

  if (benchmarkFile) {
    fprintf(stderr,"*info* opening benchmark file '%s'\n", benchmarkFile);
    bfp = fopen(benchmarkFile, "wt");
    if (!bfp) {
      perror("fsfiller");exit(1);
    }
  }


  // setup worker threads
  pthread_t *tid = calloc(threads, sizeof(pthread_t)); assert(tid);
  
  threadInfoType *threadinfo = calloc(threads, sizeof(threadInfoType)); assert(threadinfo);
  
  for (int i = 0; i < threads; i++){
    threadinfo[i].threadid = i;
    threadinfo[i].maxthreads = threads;
    threadinfo[i].writesize = writesize;
    threadinfo[i].filesize = filesize;
    threadinfo[i].totalFileSpace = totalfilespace;
    threadinfo[i].numfiles = numFiles;
    threadinfo[i].actions = actions;
    threadinfo[i].fileid = fileid;
    int error = pthread_create(&(tid[i]), NULL, &worker, &threadinfo[i]); // start threads
    if (error != 0) {
      printf("\nThread can't be created :[%s]", strerror(error)); 
    }
  }

  double starttime = timedouble();
  while (!finished) {
    if (timelimit) {
      if (timedouble() - starttime >= timelimit) {
	finished = 1;
	break;
      }
    }
    sleep(1);
  }

  for (int i = 0; i < threads; i++){
    pthread_join(tid[i], NULL);
  }

  free(threadinfo);
  free(tid);
  free(fileid);
  free(actions);

  if (bfp) {
    fclose(bfp);
  }
}
