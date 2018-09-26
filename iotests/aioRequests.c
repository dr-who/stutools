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
			     const size_t oneShot,
			     int dontExitOnErrors,
			     io_context_t *ioc,
			     size_t contextCount
			     ) {
  int ret;
  struct iocb **cbs;
  struct io_event *events;

  assert(QD);

  CALLOC(events, QD, sizeof(struct io_event));
  CALLOC(cbs, QD, sizeof(struct iocb*));
  for (size_t i = 0; i < QD; i++) {
    CALLOC(cbs[i], 1, sizeof(struct iocb));
  }

  if (verbose) {
    for (size_t i = 0; i < contextCount; i++) {
      fprintf(stderr,"*info* io_context[%zd] = %p\n", i, (void*)ioc[i]);
    }
  }
  
  
  //  ctx = 0;
  // set the queue depth

  //  ret = io_setup(QD, &ctx);
  //  if (ret != 0) {perror("io_setup");abort();}

  /* setup I/O control block, randomised just for this run. So we can check verification afterwards */
  char **data = NULL, **dataread = NULL;
  CALLOC(data, QD, sizeof(char*));
  CALLOC(dataread, QD, sizeof(char*));

  // setup the buffers to be contiguous
  CALLOC(data[0], 1, randomBufferSize * QD);
  CALLOC(dataread[0], 1, randomBufferSize * QD);
  
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
  size_t *inFlightPer;
  CALLOC(inFlightPer, contextCount, sizeof(size_t));

  size_t pos = 0;

  double start = timedouble();
  double last = start, gt = start, lastsubmit =start, lastreceive = start;
  logSpeedReset(benchl);
  logSpeedReset(alll);

  size_t submitted = 0, flushPos = 0, received = 0;
  size_t totalWriteBytes = 0, totalReadBytes = 0;
  
  size_t lastBytes = 0, lastIOCount = 0;
  struct timespec timeout;
  timeout.tv_sec = 0;
  timeout.tv_nsec = 1*1000*1000; // 0.001 seconds

  double flush_totaltime = 0, flush_mintime = 9e99, flush_maxtime = 0;
  size_t flush_count = 0;
  
  while (keepRunning) {
    if (inFlightPer[positions[pos].dev->ctxIndex] < QD) {
      
      // submit requests, one at a time
      //      fprintf(stderr,"max %zd\n", MAX(QD - inFlight, 1));
      int submitCycles = MAX(QD - inFlightPer[positions[pos].dev->ctxIndex], 1);
      if (flushEvery) {
	if (flushEvery < submitCycles) {
	  submitCycles = flushEvery;
	}
      }
      for (size_t i = 0; i < submitCycles; i++) {
	if (sz && positions[pos].action != 'S') { // if we have some positions, sz > 0
	  long long newpos = positions[pos].pos;
	  const size_t len = positions[pos].len;

	  int read = (positions[pos].action == 'R');
	  
	  // setup the request
	  if (positions[pos].dev && (positions[pos].dev->fd >= 0)) {
	    if (read) {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] read ", pos);}
	      io_prep_pread(cbs[submitted%QD], positions[pos].dev->fd, dataread[submitted%QD], len, newpos);
	      positions[pos].success = 1;
	      totalReadBytes += len;
	    } else {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] write ", pos);}
	      //assert(randomBuffer[0] == 's'); // from stutools
	      io_prep_pwrite(cbs[submitted%QD], positions[pos].dev->fd, data[submitted%QD], len, newpos);
	      positions[pos].success = 1;
	      totalWriteBytes += len;
	      flushPos++;
	    }
	    //
	    //cbs[submitted%QD]->u.c.offset = sz;
	    //	fprintf(stderr,"submit...\n");
	    
	    //	    size_t rpos = (char*)data[submitted%QD] - (char*)data[0];
	    //fprintf(stderr,"*info*submit %zd\n", rpos / randomBufferSize);
	    //	    fprintf(stderr,"submit %p\n", (void*)positions[pos].dev->ctx);
	    //	    fprintf(stderr,"%d\n", positions[pos].dev->ctxIndex);
	    //	    fprintf(stderr,"%p\n", (void*)ioc[positions[pos].dev->ctxIndex]);
	    inFlightPer[positions[pos].dev->ctxIndex]++;
	    inFlight++;
	    
	    ret = io_submit(ioc[positions[pos].dev->ctxIndex], 1, &cbs[submitted%QD]);
	    if (ret > 0) {
	      lastsubmit = timedouble(); // last good submit
	      submitted++;
	      if (verbose >= 2 || (newpos & (alignment-1))) {
		fprintf(stderr,"fd %d, pos %lld (%s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", positions[pos].dev->fd, newpos, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
	      }
	      
	    } else {
	      fprintf(stderr,"io_submit() failed.\n"); perror("io_submit()"); if(!dontExitOnErrors) abort();
	    }
	  }
	}
	// onto the next one
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
	    if (verbose != -1) fprintf(stderr,"[%.1lf] %.1lf GiB, qd: %zd, op: %zd, [%zd], %.0lf IO/s, %.1lf MiB/s\n", gt - start, TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, submitted / (gt - start), speed);
	    if (verbose) {
	      if (flush_count) fprintf(stderr,"*info* avg flush time %.4lf (min %.4lf, max %.4lf)\n", flush_totaltime / flush_count, flush_mintime, flush_maxtime);
	    }
	  }
	  last = gt;
	  if ((!keepRunning) || ((long)(gt - start) >= secTimeout)) {
	    //	    	  fprintf(stderr,"timeout\n");
	    goto endoffunction;
	  }
	}
      } // for loop i

      if (!sz) flushPos++; // if no positions, then increase flushPos anyway
      
      if (flushEvery) {
	if (flushPos >= flushEvery) {
	  flushPos = flushPos - flushEvery;
	  if (verbose >= 2) {
	    fprintf(stderr,"[%zd] SYNC: calling fsync()\n", pos);
	  }
	  double start_f = timedouble(); // time and store
	  assert(positions[pos].dev);
	  fsync(positions[pos].dev->fd);
	  double elapsed_f = timedouble() - start_f;

	  flush_totaltime += (elapsed_f);
	  flush_count++;
	  if (elapsed_f < flush_mintime) flush_mintime = elapsed_f;
	  if (elapsed_f > flush_maxtime) flush_maxtime = elapsed_f;
	}
      }
  }

    if (inFlightPer[positions[pos].dev->ctxIndex] < 5) { // then don't timeout
      ret = io_getevents(ioc[positions[pos].dev->ctxIndex], 1, QD, events, NULL);
    } else {
      ret = io_getevents(ioc[positions[pos].dev->ctxIndex], 1, QD, events, &timeout);
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

	int rescode = events[j].res;
	int rescode2 = events[j].res2;

	if ((rescode <= 0) || (rescode2 != 0)) { // if return of bytes written or read
	  fprintf(stderr,"*error* AIO failure codes: res=%d (%s) and res2=%d (%s)\n", rescode, strerror(rescode), rescode2, strerror(rescode2));
	  fprintf(stderr,"*error* last successful submission was %.3lf seconds ago\n", timedouble() - lastsubmit);
	  fprintf(stderr,"*error* last successful receive was %.3lf seconds ago\n", timedouble() - lastreceive);
	  goto endoffunction;
	  //	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
	} else {
	  lastreceive = timedouble(); // last good receive

	  // all ok
	  //	  struct iocb *my_iocb = events[j].obj;
	  //fprintf(stderr,"returned %zd\n", (char*)my_iocb->u.c.buf - (char*)(data[0]));
	}
      }
      inFlightPer[positions[pos].dev->ctxIndex] -= ret;
      inFlight -= ret;
      received += ret;
    }
    if (ret < 0) {
      //      fprintf(stderr,"eek\n");
      //    perror("io_destroy");
      break;
    }

  }

  
 endoffunction:
  // receive outstanding I/Os
  while (inFlight) {
    for (size_t i = 0; i < contextCount; i++) {
      if (verbose || inFlight)
	fprintf(stderr,"*info* inflight[%zd] = %zd\n", i, inFlightPer[i]);
      if (inFlightPer[i]) {
	int ret = io_getevents(ioc[i], inFlightPer[i], inFlightPer[i], events, NULL);
	if (ret > 0) {
	  inFlightPer[i] -= ret;
	  inFlight -= ret;
	}
      }
    }
  }

  free(events);
  for (size_t i = 0; i < QD; i++) {
    free(cbs[i]);
  }
  free(cbs);
  free(data[0]);
  free(data);
  free(dataread[0]);
  free(dataread);
  //  io_destroy(ctx);
  

  *ios = received;

  *totalWB = totalWriteBytes;
  *totalRB = totalReadBytes;
  
  return (*totalWB) + (*totalRB);
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

int aioVerifyWrites(positionType *positions,
		    const size_t maxpos,
		    const size_t maxBufferSize,
		    const size_t alignment,
		    const int verbose,
		    const char *randomBuffer) {

  fprintf(stderr,"*info* sorting verification array\n");
  qsort(positions, maxpos, sizeof(positionType), poscompare);

  size_t errors = 0, checked = 0, ioerrors = 0;
  char *buffer;
  CALLOC(buffer, maxBufferSize, 1);

  size_t bytesToVerify = 0, posTOV = 0;
  for (size_t i = 0; i < maxpos; i++) {
    /*    if (i < 10) {
      fprintf(stderr,"%d %zd %zd\n", positions[i].dev->fd, positions[i].pos, positions[i].len);
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
	//	fprintf(stderr,"\n%zd: %c %zd %zd %d\n", i, positions[i].action, positions[i].pos, positions[i].len, positions[i].success);
	checked++;
	const size_t pos = positions[i].pos;
	const size_t len = positions[i].len;
	assert(len <= maxBufferSize);
	assert(len > 0);
	
	if (lseek(positions[i].dev->fd, pos, SEEK_SET) == -1) {
	  if (errors + ioerrors < 10) {
	    fprintf(stderr,"*error* seeking to pos %zd: ", pos);
	    //	    perror("cannot seek");
	  }
	}
	//	buffer[0] = 256 - randomBuffer[0] ^ 0xff; // make sure the first char is different
	int bytesRead = read(positions[i].dev->fd, buffer, len);
	if (lseek(positions[i].dev->fd, pos, SEEK_SET) == -1) {
	  if (errors + ioerrors < 10) {
	    fprintf(stderr,"*error* seeking to pos %zd: ", pos);
	    //	    perror("cannot seek");
	  }
	}

	if ((size_t)bytesRead != len) {
	  ioerrors++;
	  if (ioerrors < 10) {
	    //	    perror("reading");
	    fprintf(stderr,"[%zd/%zd][position %zd, %zd/%zd %% %zd] verify read truncated bytesRead=%d instead of len=%zd\n", checked, maxpos, pos, pos / maxBufferSize, pos % maxBufferSize, maxBufferSize, bytesRead, len);
	  }
	} else {
	  assert(bytesRead == len);
	  if (strncmp(buffer, randomBuffer, bytesRead) != 0) {
	    errors++;	
	    if (errors < 10) {
	      char *bufferp = buffer;
	      char *randomp = (char*)randomBuffer;
	      for (size_t p = 0; p < bytesRead; p++) {
		if (*bufferp != *randomp) {
		  fprintf(stderr,"[%zd/%zd][position %zd, %zd/%zd %% %zd] verify error [size=%zd, read=%d] at location (%zd).  '%c'   '%c' \n", checked, maxpos, pos, pos / maxBufferSize, pos % maxBufferSize, maxBufferSize, len, bytesRead, p, buffer[p], randomBuffer[p]);
		  break;
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
  fprintf(stderr,"checked %zd/%zd blocks, I/O errors %zd, errors/incorrect %zd, elapsed = %.1lf secs (%.1lf MiB/s)\n", checked, posTOV, ioerrors, errors, elapsed, TOMiB(bytesToVerify)/elapsed);

  
  free(buffer);

  return ioerrors + errors;
}



