#define _GNU_SOURCE             /* See feature_test_macros(7) */

#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <assert.h>

typedef struct {
  size_t id;
  size_t max;
  int infile, outfile;
  size_t start;
  size_t len;
  size_t fileSize;
} threadInfoType;



static void *runThread(void *x) {
  threadInfoType *tc = (threadInfoType*)x;
  assert(tc);

  if (tc->len == 0 && tc->start == 0) {
    return NULL;
  }

  int sz;
  char *buffer = calloc(1024*1024, sizeof(char));
  size_t pos = tc->start;

  size_t last = 0;
  
  while (pos < tc->start + tc->len) {
    if (tc->id == 0) {
      if ((pos - last) > 10L*1024*1024) {
	fprintf(stderr,"[progress] %.1lf%%\n", pos *100.0 / (tc->start + tc->len));
	last = pos;
      }
    }
    //    fprintf(stderr,"[%d] %zd\n", tc->infile, pos);
    sz = pread(tc->infile, buffer, 1024*1024, pos);
    //    fprintf(stderr,"read %d\n", sz);
    if (sz > 0) {
      int e = pwrite(tc->outfile, buffer, sz, pos);
      if (e < 0) {
	perror("error");
	exit(1);
      }
      pos += sz;
    } else {
      //      perror("wow");
      break;
    }
  }

  free(buffer);

  return NULL;
}


void usage() {
  printf("usage: pcp <infile> <outfile>\n");
  printf("\n");
  printf("description:\n  stu's parallel file copier (32 threads)\n");
}

int main(int argc, char *argv[]) {

  size_t start = time(NULL);
  
  if (argc!= 3) {
    usage();
    exit(1);
  }

  if (strcmp(argv[1], argv[2]) == 0) {
    fprintf(stderr,"*error* in and out files can't be the same\n");
    exit(1);
  }
  int infile = open(argv[1], O_RDONLY);
  
  if (infile < 0) {
    perror(argv[1]);
    exit(1);
  }
  size_t infileSize = lseek(infile, 0L, SEEK_END);
  fprintf(stderr,"*info* infile size: %zd\n", infileSize);
  
  int outfile = open(argv[2], O_RDWR | O_CREAT | O_TRUNC,  S_IRUSR  |  S_IWUSR );
  if (outfile < 0) {
    perror(argv[1]);
    exit(1);
  }
  fallocate(outfile, 0, 0, infileSize);

  size_t num = 32;
  
  pthread_t *pt = calloc(num, sizeof(pthread_t));
  assert(pt);
  
  threadInfoType *threadContext = calloc(num, sizeof(threadInfoType));
  assert(threadContext);
  
  size_t slice = 1;
  const size_t origsize = infileSize;
  infileSize = infileSize / num;
  while (infileSize >= 1) {
    slice = slice * 2L;
    infileSize = infileSize / 2L;
  }
  for (size_t i = 0; i < num; i++) {
    threadContext[i].id = i;
    threadContext[i].max = num;
    threadContext[i].fileSize = origsize;
    threadContext[i].infile = dup(infile);
    threadContext[i].outfile = dup(outfile);
    threadContext[i].len = slice;
    threadContext[i].start = i*slice;
    if (threadContext[i].start >= threadContext[i].fileSize) {
      threadContext[i].len = 0;
      threadContext[i].start = 0;
    }

    //    fprintf(stderr,"*info* %zd, slice %zd, i*slice %zd, start %zd, end %zd\n", i, slice, i * slice, threadContext[i].start, threadContext[i].start + threadContext[i].len);
  }

  for (size_t tid = 0; tid < num; tid++) {
    pthread_create(&(pt[tid]), NULL, runThread, &(threadContext[tid]));
  }
  for (size_t i = 0; i < num; i++) {
    pthread_join(pt[i], NULL);
  }

  
  
  close(infile);
  close(outfile);

  size_t finish = time(NULL);
  fprintf(stderr,"*info* %zd bytes written in %zd seconds = %.3lf GB/s\n", origsize, finish - start, origsize / (finish - start) / 1000.0/1000/1000);
  
  return 0;
}
