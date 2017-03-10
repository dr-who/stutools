#define _GNU_SOURCE

#include <stdlib.h>
#include <malloc.h>
#include <unistd.h>
#include <fcntl.h>
#include <libaio.h>

#include "utils.h"

extern int keepRunning;

#define MAXDEPTH 64

long readNonBlocking(const char *path, const size_t BLKSIZE, const size_t sz, const float secTimeout) {
  io_context_t ctx;
  struct iocb *cbs[1];
  char *data;
  struct io_event events[MAXDEPTH];
  int ret;
  int fd;

  fprintf(stderr,"read %s: %.1lf GiB (%.0lf MiB), blocksize %zd B (%zd KiB), timeout %.1f s\n", path, sz / 1024.0 / 1024 / 1024, sz / 1024.0 / 1024 , BLKSIZE, BLKSIZE / 1024, secTimeout);

  fd = open(path, O_RDONLY | O_EXCL | O_DIRECT);
  if (fd < 0) {perror(path);return -1; }

  ctx = 0;

  size_t inFlight = 0;

  // set the queue depth
  ret = io_setup(MAXDEPTH, &ctx);
  if (ret < 0) {perror("io_setup");fsync(fd);close(fd); return -1;}

  /* setup I/O control block */
  data = aligned_alloc(4096, BLKSIZE); if (!data) {perror("oom"); exit(1);}
  memset(data, 'A', BLKSIZE);

  cbs[0] = malloc(sizeof(struct iocb));

  size_t maxBlocks = sz / BLKSIZE;
  srand(sz + fd);

  double start = timedouble();
  size_t bytesReceived = 0;
  //  size_t count = 0;

  while (1) {
    if (inFlight < MAXDEPTH) {
      
      
      // submit requests
      for (size_t i = 0; i < (MAXDEPTH - inFlight); i++) {
	size_t newpos = (rand() % maxBlocks) * BLKSIZE;
	//size_t newpos = ((count++) % maxBlocks) * BLKSIZE;
	if (newpos > sz) {
	  newpos = newpos % sz; // set to zero and warn
	  fprintf(stderr,"newpos truncated to 0\n");
	}
	// setup the read request
	io_prep_pread(cbs[0], fd, data, BLKSIZE, newpos);

	//    cbs[0]->u.c.offset = sz;
	ret = io_submit(ctx, 1, cbs);
	inFlight++;
	
	if ((!keepRunning) || (timedouble() - start > secTimeout)) {
	  //	  fprintf(stderr,"timeout\n");
	  goto retread;
	}
	if (ret != 1) {
	  fprintf(stderr,"eek i=%zd %d\n", i, ret);
	} else {
	  //      fprintf(stderr,"red %d\n", ret);
	}
      }
    }

    ret = io_getevents(ctx, 0, MAXDEPTH, events, NULL);

    if ((!keepRunning) || (timedouble() - start > secTimeout)) {
      break;
    }

    if (ret > 0) {
      inFlight-= ret;

      //      fprintf(stderr,"in flight %zd\n", inFlight);
      
      bytesReceived += (ret * BLKSIZE);
      //      fprintf(stderr, "events: ret %d, seen %zd, total %zd\n", ret, seen, bytesReceived);
    } else {
      //            fprintf(stderr,".");
      usleep(1);
    }
    //	  ret = io_destroy(ctx);
    if (ret < 0) {
      perror("io_destroy");
      break;
    }
  }
 retread:
  fsync(fd);
  int retc = close(fd);
  if (retc != 0) {
    perror("read closing error");
  }
  return bytesReceived;
}






long writeNonBlocking(const char *path, const size_t BLKSIZE, const size_t sz, const float secTimeout) {
  io_context_t ctx;
  struct iocb *cbs[1];
  char *data;
  struct io_event events[MAXDEPTH];
  int ret;
  int fd;

  fprintf(stderr,"write %s: %.1lf GiB (%.0lf MiB), blocksize %zd B (%zd KiB), timeout %.1f s\n", path, sz / 1024.0 / 1024 / 1024, sz / 1024.0 / 1024 , BLKSIZE, BLKSIZE / 1024, secTimeout);

  fd = open(path, O_WRONLY | O_EXCL | O_DIRECT);
  if (fd < 0) {perror(path);return -1; }

  ctx = 0;

  // set the queue depth
  ret = io_setup(MAXDEPTH, &ctx);
  if (ret < 0) {perror("io_setup");return -1;}

  /* setup I/O control block */
  data = aligned_alloc(4096, BLKSIZE); if (!data) {perror("oom"); exit(1);}
  memset(data, 'A', BLKSIZE);
  cbs[0] = malloc(sizeof(struct iocb));


  // submit requests
  size_t maxBlocks = sz / BLKSIZE;
  srand(sz + fd);


  double start = timedouble();
  size_t bytesReceived = 0;
  size_t inFlight = 0;
  //  size_t count = 0;
  size_t gotenough = 0;

  while (1) {
    if (inFlight < MAXDEPTH) {

      for (size_t i = 0 ; i < (MAXDEPTH - inFlight) ; i++) {
	size_t newpos = (rand() % maxBlocks) * BLKSIZE;
	//size_t newpos = ((count++) % maxBlocks) * BLKSIZE;
	if (newpos > sz) {
	  newpos = newpos % sz; // set to zero and warn
	  fprintf(stderr,"newpos truncated to 0\n");
	}
	// setup the read request
	io_prep_pwrite(cbs[0], fd, data, BLKSIZE, newpos);
	
	//    cbs[0]->u.c.offset = sz;
	ret = io_submit(ctx, 1, cbs);
	inFlight++;
	
	if ((!keepRunning) || ((gotenough && (timedouble() - start > secTimeout))) ) {
	  //	  fprintf(stderr,"timeout2\n");
	  goto retwrite;
	}
	if (ret != 1) {
	  fprintf(stderr,"eek i=%zd %d\n", i, ret);
	} else {
	  //      fprintf(stderr,"red %d\n", ret);
	}
      }
    }
    
    ret = io_getevents(ctx, 0, MAXDEPTH, events, NULL);
    
    if ( (!keepRunning) || ((gotenough && (timedouble() - start > secTimeout))) ) {
      break;
    }
    
    if (ret > 0) {
      inFlight -= ret;
      
      bytesReceived += (ret * BLKSIZE);
      if (!gotenough && (bytesReceived > 64*1024*1024)) { // ignore first 64MiB
	//	fprintf(stderr,"resetting\n");
	gotenough = 1;
	bytesReceived = 0;
	start = timedouble();
      }
      //      fprintf(stderr, "events: ret %d, seen %zd, total %zd\n", ret, seen, bytesReceived);
    } else {
      //            fprintf(stderr,".");
      usleep(1);
    }
    //	  ret = io_destroy(ctx);
    if (ret < 0) {
      perror("io_destroy");
      break;
    }
  }
 retwrite:
  fsync(fd);
  int retc = close(fd);
  if (retc != 0) {
    perror("write closing error");
  }
  return bytesReceived;
}
