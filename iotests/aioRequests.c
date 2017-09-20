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
  double last = start, gt = start;
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
	    io_prep_pread(cbs[0], positions[pos].fd, data[i%QD], len, newpos);
	    positions[pos].success = 1;
	    totalReadBytes += len;
	  } else {
	    if (verbose >= 2) {fprintf(stderr,"[%zd] write ", pos);}
	    assert(randomBuffer[0] == 's'); // from stutools
	    io_prep_pwrite(cbs[0], positions[pos].fd, randomBuffer, len, newpos);
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
	      fprintf(stderr,"fd %d, pos %lld (%s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", positions[pos].fd, newpos, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
	    }
	    
	  } else {
	    fprintf(stderr,"io_submit() failed.\n"); perror("io_submit()");exit(1);
	  }
	  
	  pos++; if (pos >= sz) pos = 0; // don't go over the end of the array
	}
	
	gt = timedouble();

	if (gt - last >= DISPLAYEVERY) {
	  if (!tableMode) fprintf(stderr,"[%.1lf] submitted %.1lf GiB, in flight/queue: %zd, received=%zd, index=%zd, %.0lf IO/sec, %.1lf MiB/sec\n", gt - start, TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, submitted / (gt - start), (totalReadBytes + totalWriteBytes) / (gt - start)/1024.0/1024);
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
    
    ret = io_getevents(ctx, 0, QD, events, NULL);

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
	if (l) {
	  logSpeedAdd(l, events[j].res);
	}
	  
	if (events[j].res <= 0) { // if return of bytes written or read
	  fprintf(stderr,"failure: %ld bytes\n", events[j].res);
	  //	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
	}
      }
	//}
      
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

  fprintf(stderr,"*info* sorting verification array, %zd positions\n", maxpos);
  qsort(positions, maxpos, sizeof(positionType), poscompare);

  size_t errors = 0, checked = 0;
  char *buffer = aligned_alloc(alignment, maxBufferSize); if (!buffer) {fprintf(stderr,"oom!!!\n");exit(1);}

  size_t bytesToVerify = 0;
  for (size_t i = 0; i < maxpos; i++) {
    /*    if (i < 10) {
      fprintf(stderr,"%d %zd %zd\n", positions[i].fd, positions[i].pos, positions[i].len);
      }*/
    if (positions[i].success) {
      if (positions[i].action == 'W') {
	bytesToVerify += positions[i].len;
      }
    }
  }

  double start = timedouble();
  fprintf(stderr,"*info* started verification (%.1lf GiB)\n", TOGiB(bytesToVerify));

  for (size_t i = 0; i < maxpos; i++) {
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
	  errors++;
	  fprintf(stderr,"[%zd/%zd][position %zd] verify read truncated bytesRead=%d instead of len=%zd\n", checked, maxpos, pos, bytesRead, len);
	} else {
	  assert(bytesRead == len);
	  assert((len % alignment) == 0);
	  if (strncmp(buffer, randomBuffer, bytesRead) != 0) {
	    errors++;
	    size_t firstprint = 0;
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
	    /*	    if (errors <= 1) {
	      fprintf(stderr,"Shouldbe: \n%s\n", randomBuffer);
	      fprintf(stderr,"ondisk:   \n%s\n", buffer);
	      }*/
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
  fprintf(stderr,"verified %zd blocks, number of errors %zd, elapsed = %.1lf secs (%.1lf MiB/s)\n", checked, errors, elapsed, TOMiB(bytesToVerify)/elapsed);

  
  free(buffer);

  return errors;
}
