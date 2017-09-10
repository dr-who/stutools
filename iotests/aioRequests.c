#define _GNU_SOURCE

#include <stdlib.h>
#include <malloc.h>
#include <unistd.h>
#include <fcntl.h>
#include <libaio.h>
#include <assert.h>

#include "utils.h"
#include "logSpeed.h"
#include "aioRequests.h"

extern int keepRunning;
extern int singlePosition;
extern int flushEvery;

size_t aioMultiplePositions(const int fd,
			     positionType *positions,
			     const size_t sz,
			     const float secTimeout,
			     const size_t QD,
			     const int verbose,
			     const int tableMode, 
			     logSpeedType *l,
			     char *randomBuffer,
			     const size_t randomBufferSize,
			     const size_t alignment,
			     size_t *ios,
			     size_t *totalRB,
			     size_t *totalWB
			     ) {
  int ret;
  io_context_t ctx;
  struct iocb *cbs[1];
  struct io_event *events;

  assert(QD);

  CALLOC(events, QD, sizeof(struct io_event));
  CALLOC(cbs[0], 1, sizeof(struct iocb));
  
  ctx = 0;

  // set the queue depth

  ret = io_setup(QD, &ctx);
  if (ret != 0) {perror("io_setup");abort();}

  /* setup I/O control block, randomised just for this run. So we can check verification afterwards */
  char **data = NULL;
  CALLOC(data, QD, sizeof(char*));

  // copy the randomBuffer to each data[]
  for (size_t i = 0; i <QD; i++) {
    data[i] = aligned_alloc(alignment, randomBufferSize); if (!data[i]) {perror("oom"); exit(1);}
    memcpy(data[i], randomBuffer, randomBufferSize);
  }

  size_t inFlight = 0;

  size_t pos = 0;

  double start = timedouble();
  double last = start;
  size_t submitted = 0;
  size_t received = 0;

  size_t totalWriteBytes = 0; 
  size_t totalReadBytes = 0; 
  //  double mbps = 0;
  

  while (1) {
    if (inFlight < QD) {
      
      // submit requests, one at a time
      for (size_t i = 0; i < MIN(QD - inFlight, 1); i++) {
	if (sz) { // if we have some positions
	  long long newpos = positions[pos].pos;
	  const size_t len = positions[pos].len;
	  int read = (positions[pos].action == 'R');
	  
	  // setup the request
	  if (read) {
	    if (verbose >= 2) {fprintf(stderr,"[%zd] read ", pos);}
	    io_prep_pread(cbs[0], fd, data[i%QD], len, newpos);
	    positions[pos].success = 1;
	    totalReadBytes += len;
	  } else {
	    if (verbose >= 2) {fprintf(stderr,"[%zd] write ", pos);}
	    assert(randomBuffer[0] == 's'); // from stutools
	    io_prep_pwrite(cbs[0], fd, randomBuffer, len, newpos);
	    positions[pos].success = 1;
	    totalWriteBytes += len;
	  }
	  //    cbs[0]->u.c.offset = sz;
	  //	fprintf(stderr,"submit...\n");
	  
	  ret = io_submit(ctx, 1, cbs);
	  if (ret > 0) {
	    inFlight++;
	    submitted++;
	    if (verbose >= 2 || (newpos % alignment != 0)) {
	      fprintf(stderr,"pos %lld (%s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", newpos, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
	    }
	    
	  } else {
	    fprintf(stderr,"!!!\n");
	  }
	  
	  pos++; if (pos >= sz) pos = 0; // don't go over the end of the array
	}
	
	double gt = timedouble();

	if (gt - last >= 1) {
	  if (!tableMode) fprintf(stderr,"submitted %.1lf GiB, in flight/queue: %zd, received=%zd, index=%zd, %.0lf IO/sec, %.1lf MiB/sec\n", TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, submitted / (gt - start), (totalReadBytes + totalWriteBytes) / (gt - start)/1024.0/1024);
	  last = gt;
	  if ((!keepRunning) || (gt - start > secTimeout)) {
	    //	  fprintf(stderr,"timeout\n");
	    goto endoffunction;
	  }
	}
	/*	if (ret != 1) {
	  fprintf(stderr,"eek i=%zd %d\n", i, ret);
	} else {
	  //      fprintf(stderr,"red %d\n", ret);
	  }*/
      } // for loop
      if (flushEvery) {
	if ((pos % flushEvery) == 0) {
	  // sync whenever the queue is full
	  if (verbose >= 2) {
	    fprintf(stderr,"[%zd] SYNC: calling fsync()\n", pos);
	  }
	  fsync(fd);
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

      // verify it's all ok
      for (int j = 0; j < ret; j++) {
	//	struct iocb *my_iocb = events[j].obj;
	if (l) {
	  logSpeedAdd(l, events[j].res);
	}
	  
	/*	if (events[j].res != len) { // if return of bytes written or read
	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
	  } */
      }
      
	/*      if (l) {
	logSpeedAdd(l, ret * len);
	}*/
	
      inFlight -= ret;
      received += ret;
    } else {
      // busy loop, no pausing here
      //      usleep(1);
    }
    //	  ret = io_destroy(ctx);
    if (ret < 0) {
      perror("io_destroy");
      break;
    }
  }

  //  mbps = received* BLKSIZE / (timedouble() - start)/1024.0/1024.0;
  
 endoffunction:
  //  fdatasync(fd);
  free(events);
  free(cbs[0]);
  for (size_t i = 0; i <QD; i++) {
    free(data[i]);
  }
  free(data);
  io_destroy(ctx);

  *ios = received;

  *totalWB = totalWriteBytes;
  *totalRB = totalReadBytes;
    
  
  return *totalWB + *totalRB;
}


int aioVerifyWrites(const char *path,
		    const positionType *positions,
		    const size_t maxpos,
		    const size_t maxBufferSize,
		    const size_t alignment,
		    const int verbose,
		    const char *randomBuffer) {


  const int fd = open(path, O_RDONLY | O_EXCL | O_DIRECT);
  if (fd < 0) {fprintf(stderr,"fd error\n");exit(1);}

  fprintf(stderr,"*info* started verification\n");

  size_t errors = 0, checked = 0;
  int bytesRead = 0;

  char *buffer = aligned_alloc(alignment, maxBufferSize); if (!buffer) {fprintf(stderr,"oom!!!\n");exit(1);}
  
  for (size_t i = 0; i < maxpos; i++) {
    if (positions[i].success) {
      if (positions[i].action == 'W') {
	assert(positions[i].len >= 1024);
	assert(positions[i].len % 1024 == 0);
	//	fprintf(stderr,"\n%zd: %c %zd %zd %d\n", i, positions[i].action, positions[i].pos, positions[i].len, positions[i].success);
	checked++;
	const size_t pos = positions[i].pos;
	const size_t len = positions[i].len;
	if (lseek(fd, pos, SEEK_SET) == -1) {
	  perror("cannot seek");
	}
	buffer[0] = randomBuffer[0] ^ 0xff; // make sure the first char is different
	
	bytesRead = read(fd, buffer, len);
	//	fprintf(stderr,"reading... %d\n", bytesRead);
	//	buffer[len - 1] = 0; // clear the end byte
	if ((size_t)bytesRead != len) {
	  fprintf(stderr,"[%zd/%zd] verify read truncated bytesRead %d instead of len=%zd\n", i, pos, bytesRead, len);
	  errors++;
	} else {
	  if (strncmp(buffer, randomBuffer, bytesRead) != 0) {
	    size_t firstprint = 1;
	    for (size_t p = 0; p < bytesRead; p++) {
	      if (buffer[p] != randomBuffer[p]) {
		if (firstprint) {
		  fprintf(stderr,"[%zd/%zd][%zd] verify error at location.  %c   %c \n", i, pos, p, buffer[p], randomBuffer[p]);
		  firstprint = 0;
		}
	      }
	    }
	    if (errors <= 1) {
	      fprintf(stderr,"Shouldbe: \n%s\n", randomBuffer);
	      fprintf(stderr,"ondisk:   \n%s\n", buffer);
	    }
	    errors++;
	  } else {
	    if (verbose >= 2) {
	      fprintf(stderr,"[%zd] verified ok location %zd, size %zd\n", i, pos, len);
	    }
	  }
	}
      }
    }
  }
  fprintf(stderr,"verified %zd blocks, number of errors %zd\n", checked, errors);
  close(fd);
  free(buffer);
  return 0;
}
