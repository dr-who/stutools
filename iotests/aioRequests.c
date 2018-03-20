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

#define DISPLAYEVERY 1

size_t aioMultiplePositions( positionType *positions,
			     const size_t sz,
			     const float secTimeout,
			     const size_t QD,
			     const int verbose,
			     const int tableMode, 
			     logSpeedType *alll,
			     logSpeedType *benchl,
			     char *randomBuffer,
			     const size_t randomBufferSize,
			     const size_t alignment,
			     size_t *ios,
			     size_t *totalRB,
			     size_t *totalWB,
			     const size_t oneShot
			     ) {
  int ret;
  io_context_t ctx;
  struct iocb **cbs;
  struct io_event *events;

  assert(QD);

  CALLOC(events, QD, sizeof(struct io_event));
  CALLOC(cbs, QD, sizeof(struct iocb*));
  for (size_t i = 0; i < QD; i++) {
    CALLOC(cbs[i], 1, sizeof(struct iocb));
 }
  
  ctx = 0;
  // set the queue depth

  ret = io_setup(QD, &ctx);
  if (ret != 0) {perror("io_setup");abort();}

  /* setup I/O control block, randomised just for this run. So we can check verification afterwards */
  char **data = NULL, **dataread = NULL;
  CALLOC(data, QD, sizeof(char*));
  CALLOC(dataread, QD, sizeof(char*));

  // setup the buffers to be contiguous
  data[0] = aligned_alloc(alignment, randomBufferSize * QD); if (!data[0]) {perror("oom"); exit(-1);}
  dataread[0] = aligned_alloc(alignment, randomBufferSize * QD); if (!dataread[0]) {perror("oom"); exit(-1);}
  for (size_t i = 0; i <QD; i++) {
    data[i] = data[0] + (randomBufferSize * i);
    dataread[i] = dataread[0] + (randomBufferSize * i);
  }

  // copy the randomBuffer to each data[]
  for (size_t i = 0; i < QD; i++) {
    if (verbose >= 2) {
      fprintf(stderr,"randomBuffer[%zd]: %p\n", i, (void*)data[i]);
    }
    strncpy(data[i], randomBuffer, randomBufferSize);
    strncpy(dataread[i], randomBuffer, randomBufferSize);
  }

  size_t inFlight = 0;

  size_t pos = 0;

  double start = timedouble();
  double last = start, gt = start;
  logSpeedReset(benchl);
  logSpeedReset(alll);
  size_t submitted = 0;
  size_t received = 0;

  size_t totalWriteBytes = 0; 
  size_t totalReadBytes = 0;
  //  double mbps = 0;
  
  size_t lastBytes = 0, lastIOCount = 0;
  struct timespec timeout;
  timeout.tv_sec = 0;
  timeout.tv_nsec = 1*1000*1000; // 0.001 seconds

  
  while (keepRunning) {
    if (inFlight < QD) {
      
      // submit requests, one at a time
      //      fprintf(stderr,"max %zd\n", MAX(QD - inFlight, 1));
      for (size_t i = 0; i < MAX(QD - inFlight, 1); i++) {
	if (sz && positions[pos].action != 'S') { // if we have some positions
	  long long newpos = positions[pos].pos;
	  const size_t len = positions[pos].len;

	  int read = (positions[pos].action == 'R');
	  
	  // setup the request
	  if (positions[pos].fd >= 0) {
	    if (read) {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] read ", pos);}
	      io_prep_pread(cbs[submitted%QD], positions[pos].fd, dataread[submitted%QD], len, newpos);
	      positions[pos].success = 1;
	      totalReadBytes += len;
	    } else {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] write ", pos);}
	      //assert(randomBuffer[0] == 's'); // from stutools
	      io_prep_pwrite(cbs[submitted%QD], positions[pos].fd, data[submitted%QD], len, newpos);
	      positions[pos].success = 1;
	      totalWriteBytes += len;
	    }
	    //
	    //cbs[submitted%QD]->u.c.offset = sz;
	    //	fprintf(stderr,"submit...\n");
	    

	    //	    size_t rpos = (char*)data[submitted%QD] - (char*)data[0];
	    //fprintf(stderr,"*info*submit %zd\n", rpos / randomBufferSize);
	    ret = io_submit(ctx, 1, &cbs[submitted%QD]);
	    if (ret > 0) {
	      inFlight++;
	      submitted++;
	      if (verbose >= 2 || (newpos % alignment != 0)) {
		fprintf(stderr,"fd %d, pos %lld (%s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", positions[pos].fd, newpos, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
	      }
	      
	    } else {
	      fprintf(stderr,"io_submit() failed.\n"); perror("io_submit()");exit(-1);
	    }
	  }
	  
	}
	pos++;
	if (pos >= sz) {
	  if (oneShot) {
	    goto endoffunction; // only go through once
	  }
	  pos = 0; // don't go over the end of the array
	}
	
	gt = timedouble();

	if (gt - last >= DISPLAYEVERY) {
	  const double speed = 1.0*(totalReadBytes + totalWriteBytes) / (gt - start)/1024.0/1024;
	  if (benchl) logSpeedAdd2(benchl, TOMiB(totalReadBytes + totalWriteBytes - lastBytes), (received - lastIOCount));
	  lastBytes = totalReadBytes + totalWriteBytes;
	  lastIOCount = received;
	  if (!tableMode) {
	    if (verbose != -1) fprintf(stderr,"[%.1lf] submitted %.1lf GiB, in flight/queue: %zd, received=%zd, index=%zd, %.0lf IO/sec, %.1lf MiB/sec\n", gt - start, TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, submitted / (gt - start), speed);
	  }
	  last = gt;
	  if ((!keepRunning) || ((long)(gt - start) >= secTimeout)) {
	    //	    	  fprintf(stderr,"timeout\n");
	    goto endoffunction;
	  }
	}
	/*	if (ret != 1) {
	  fprintf(stderr,"eek i=%zd %d\n", i, ret);
	} else {
	  //      fprintf(stderr,"red %d\n", ret);
	  }*/
      } // for loop i
      
      if (flushEvery) {
	if ((pos % flushEvery) == 0) {
	  // sync whenever the queue is full
	  if (verbose >= 2) {
	    fprintf(stderr,"[%zd] SYNC: calling fsync()\n", pos);
	  }
	  fsync(positions[pos].fd);
	}
      }
    }

    if (QD < 5) { // then poll
      ret = io_getevents(ctx, 1, QD, events, NULL);
    } else {
      ret = io_getevents(ctx, 1, QD, events, &timeout);
    }

    // if stop running or been running long enough
    if ((!keepRunning) || ((long)(gt - start) >= secTimeout)) {
      if (gt - last > 1) { // if it's been over a second since per-second output
	if (received > 0) {
	  break;
	}
      }
    }

    if (ret > 0) {
      // verify it's all ok
      for (int j = 0; j < ret; j++) {
	//	struct iocb *my_iocb = events[j].obj;
	if (alll) logSpeedAdd2(alll, TOMiB(events[j].res), 1);
	  
	if ((events[j].res <= 0) || (events[j].res2 != 0)) { // if return of bytes written or read
	  fprintf(stderr,"failure: %ld bytes\n", events[j].res);
	  goto endoffunction;
	  //	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
	} else {
	  //	  struct iocb *my_iocb = events[j].obj;
	  //fprintf(stderr,"returned %zd\n", (char*)my_iocb->u.c.buf - (char*)(data[0]));
	}
      }
      inFlight -= ret;
      received += ret;
    }
    //	  ret = io_destroy(ctx);
    if (ret < 0) {
      //      perror("io_destroy");
      break;
    }
  }

  //  mbps = received* BLKSIZE / (timedouble() - start)/1024.0/1024.0;
  
 endoffunction:
  free(events);
  for (size_t i = 0; i < QD; i++) {
    free(cbs[i]);
  }
  free(data[0]);
  free(data);
  free(dataread[0]);
  free(dataread);
  io_destroy(ctx);

  *ios = received;

  *totalWB = totalWriteBytes;
  *totalRB = totalReadBytes;
  
  return *totalWB + *totalRB;
}


// sorting function, used by qsort
static int poscompare(const void *p1, const void *p2)
{
  const positionType *pos1 = (positionType*)p1;
  const positionType *pos2 = (positionType*)p2;

  if (pos1->pos < pos2->pos) return -1;
  else if (pos1->pos > pos2->pos) return 1;
  else return 0;
}

int aioVerifyWrites(int *fdArray,
		    const size_t fdlen,
		    positionType *positions,
		    const size_t maxpos,
		    const size_t maxBufferSize,
		    const size_t alignment,
		    const int verbose,
		    const char *randomBuffer) {

  fprintf(stderr,"*info* sorting verification array\n");
  qsort(positions, maxpos, sizeof(positionType), poscompare);

  size_t errors = 0, checked = 0, ioerrors = 0;
  char *buffer = aligned_alloc(alignment, maxBufferSize); if (!buffer) {fprintf(stderr,"oom!!!\n");exit(-1);}

  size_t bytesToVerify = 0, posTOV = 0;
  for (size_t i = 0; i < maxpos; i++) {
    /*    if (i < 10) {
      fprintf(stderr,"%d %zd %zd\n", positions[i].fd, positions[i].pos, positions[i].len);
      }*/
    if (positions[i].success) {
      if (positions[i].action == 'W') {
	bytesToVerify += positions[i].len;
	posTOV++;
      }
    }
  }

  double start = timedouble();
  fprintf(stderr,"*info* started verification (%zd positions, %.1lf GiB)\n", posTOV, TOGiB(bytesToVerify));

  for (size_t i = 0; i < maxpos; i++) {
    if (!keepRunning) break;
    if (positions[i].success) {
      if (positions[i].action == 'W') {
	//	assert(positions[i].len >= 1024);
	//	assert(positions[i].len % 1024 == 0);
	//	fprintf(stderr,"\n%zd: %c %zd %zd %d\n", i, positions[i].action, positions[i].pos, positions[i].len, positions[i].success);
	checked++;
	const size_t pos = positions[i].pos;
	const size_t len = positions[i].len;
	assert((len % alignment) == 0);
	assert(len <= maxBufferSize);
	assert(len > 0);
	assert((pos % alignment) == 0);
	
	if (lseek(positions[i].fd, pos, SEEK_SET) == -1) {
	  perror("cannot seek");
	}
	//	buffer[0] = 256 - randomBuffer[0] ^ 0xff; // make sure the first char is different
	const int bytesRead = read(positions[i].fd, buffer, len);

	if ((size_t)bytesRead != len) {
	  ioerrors++;
	  if (ioerrors < 10) 
	    fprintf(stderr,"[%zd/%zd][position %zd] verify read truncated bytesRead=%d instead of len=%zd\n", checked, maxpos, pos, bytesRead, len);
	} else {
	  assert(bytesRead == len);
	  assert((len % alignment) == 0);
	  if (strncmp(buffer, randomBuffer, bytesRead) != 0) {
	    errors++;	
	    size_t firstprint = 0;
	    if (errors < 10) {
	      char *bufferp = buffer;
	      char *randomp = (char*)randomBuffer;
	      for (size_t p = 0; p < bytesRead; p++) {
		if (*bufferp != *randomp) {
		  //	      if (buffer[p] != randomBuffer[p]) {
		  if (firstprint < 1) {
		    fprintf(stderr,"[%zd/%zd][position %zd] verify error [size=%zd, read=%d] at location (%zd).  '%c'   '%c' \n", checked, maxpos, pos, len, bytesRead, p, buffer[p], randomBuffer[p]);
		    firstprint++;
		  }
		}
		bufferp++;
		randomp++;
	      }
	    }
	  } else {
	    if (verbose >= 2) {
	      fprintf(stderr,"[%zd] verified ok location %zd, size %zd\n", checked, pos, len);
	    }
	  }
	}
      }
    }
  }
  double elapsed = timedouble() - start;
  fprintf(stderr,"verified %zd/%zd blocks, io errors %zd, differences %zd, elapsed = %.1lf secs (%.1lf MiB/s)\n", checked, posTOV, ioerrors, errors, elapsed, TOMiB(bytesToVerify)/elapsed);

  
  free(buffer);

  return errors;
}
