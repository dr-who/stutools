#define _GNU_SOURCE

#include <stdlib.h>
#include <malloc.h>
#include <unistd.h>
#include <fcntl.h>
#include <libaio.h>
#include <assert.h>

#include "utils.h"

extern int keepRunning;

#define MIN(x,y) (((x) < (y)) ? (x) : (y))

double readMultiplePositions(const int fd,
			   const size_t *positions,
			   const size_t sz,
			   const size_t BLKSIZE,
			   const float secTimeout,
			   const size_t QD,
			   const double readRatio,
			   const int verbose) {
  int ret;
  io_context_t ctx;
  struct iocb *cbs[1];
  struct io_event *events;

  events = malloc(QD * sizeof(struct io_event)); assert(events);
  cbs[0] = malloc(sizeof(struct iocb));

  //  cbs = malloc(sizeof(struct iocb*) * QD); assert(cbs);
  //for (size_t i = 0 ; i < QD; i++) {
  //    cbs[i] = malloc(sizeof(struct iocb)); assert(cbs[i]);
  //  }

  
  //  fprintf(stderr,"write %s: %.1lf GiB (%.0lf MiB), blocksize %zd B (%zd KiB), timeout %.1f s\n", path, sz / 1024.0 / 1024 / 1024, sz / 1024.0 / 1024 , BLKSIZE, BLKSIZE / 1024, secTimeout);

  ctx = 0;

  // set the queue depth
  ret = io_setup(QD, &ctx);
  if (ret < 0) {perror("io_setup");return -1;}

  /* setup I/O control block */
  char **data = malloc(QD * sizeof(char*));
  for (size_t i = 0; i <QD; i++) {
    data[i] = aligned_alloc(4096, BLKSIZE); if (!data[i]) {perror("oom"); exit(1);}
    memset(data[i], 'A', BLKSIZE);
  }

  int inFlight = 0;

  size_t pos = 0;

  double start = timedouble();
  double last = start;
  size_t submitted = 0;
  size_t received = 0;
  //  double mbps = 0;
  

  while (1) {
    if (inFlight < QD) {
      
      // submit requests, one at a time
      for (size_t i = 0; i < MIN(QD - inFlight, 1); i++) {
	size_t newpos = positions[pos++]; if (pos > sz) pos = 0;
	//	fprintf(stderr,"ewpos %zd\n", newpos);
	// setup the read request
	if ((readRatio >= 1.0) || (lrand48()%100 < 100*readRatio)) {
	  io_prep_pread(cbs[0], fd, data[i%QD], BLKSIZE, newpos);
	  //	  fprintf(stderr,"r");
	} else {
	  io_prep_pwrite(cbs[0], fd, data[i%QD], BLKSIZE, newpos);
	  //	  	  fprintf(stderr,"w");
	}
	

	//    cbs[0]->u.c.offset = sz;
	//	fprintf(stderr,"submit...\n");
	ret = io_submit(ctx, 1, cbs);
	if (ret == 1) {
	  inFlight++;
	  submitted++;
	} else {
	  fprintf(stderr,"!!!\n");
	}
	
	double gt = timedouble();

	if (gt - last >= secTimeout) {
	  if (verbose) fprintf(stderr,"submitted %zd, in flight/queue: %d, received=%zd, pos=%zd, %.0lf IO/sec, %.1lf MiB/sec\n", submitted, inFlight, received, pos, submitted / (gt - start), received* BLKSIZE / (gt - start)/1024.0/1024);
	  last = gt;
	  if ((!keepRunning) || (gt - start > secTimeout)) {
	    //	  fprintf(stderr,"timeout\n");
	    goto endoffunction;
	  }
	}
	if (ret != 1) {
	  fprintf(stderr,"eek i=%zd %d\n", i, ret);
	} else {
	  //      fprintf(stderr,"red %d\n", ret);
	}
      }
    }

    
    ret = io_getevents(ctx, 0, QD, events, NULL);

    if ((!keepRunning) || (timedouble() - start > secTimeout)) {
      if (received > 0) {
	break;
      }
    }

    if (ret > 0) {
      inFlight -= ret;

      //      fprintf(stderr,"in flight %zd\n", inFlight);
      
      //      bytesReceived += (ret * BLKSIZE);
      //      fprintf(stderr, "events: ret %d, seen %zd, total %zd\n", ret, seen, bytesReceived);
      received += ret;
      //      }
	
    } else {
      //             fprintf(stderr,".");
      //		  usleep(1);
    }
    //	  ret = io_destroy(ctx);
    if (ret < 0) {
      perror("io_destroy");
      break;
    }
  }

  //  mbps = received* BLKSIZE / (timedouble() - start)/1024.0/1024.0;
  
 endoffunction:
  io_destroy(ctx);
  return received;
}
