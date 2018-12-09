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

extern volatile int keepRunning;

#define DISPLAYEVERY 1

size_t aioMultiplePositions( positionType *positions,
			     const size_t sz,
			     const double finishtime,
			     const size_t origQD,
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
			     const int dontExitOnErrors,
			     const int fd,
			     int flushEvery
			     ) {
  int ret;
  struct iocb **cbs;
  struct io_event *events;
  size_t QD = origQD;

  io_context_t ioc = 0;
  io_setup(QD, &ioc);
  
  assert(QD);

  CALLOC(events, QD, sizeof(struct io_event));
  CALLOC(cbs, QD, sizeof(struct iocb*));
  for (size_t i = 0; i < QD; i++) {
    CALLOC(cbs[i], 1, sizeof(struct iocb));
  }

  if (verbose >= 1) {
    for (size_t i = 0; i < 1; i++) {
      fprintf(stderr,"*info* io_context[%zd] = %p\n", i, (void*)ioc);
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
  CALLOC(data[0], randomBufferSize * QD, 1);
  CALLOC(dataread[0], randomBufferSize * QD, 1);

  long *pointtopos; // the data structure that maps the offset back to a QD
  CALLOC(pointtopos, randomBufferSize * QD, sizeof(long));
  long *pointtoposread; // the data structure that maps the offset back to a QD
  CALLOC(pointtoposread, randomBufferSize * QD, sizeof(long));

  size_t *posInFlight; // the data struction that maps QD index to the positions[] array
  CALLOC(posInFlight, QD, 1);
  size_t *stillInFlight; // qd collisions
  CALLOC(stillInFlight, QD, 1);

  size_t *freeQueue; // qd collisions
  size_t headOfQueue = 0;
  size_t tailOfQueue = 0;
  CALLOC(freeQueue, QD, 1);
  for (size_t i = 0; i < QD; i++) {
    freeQueue[i] = i;
  }
  // grab [headOfQueue], put back onto [tailOfQueue]

  // there are 256 queue slots
  // O(1) next queue slot
  // take a slot, free a slot
  // free list
  // qd[..255] in flight
  
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

  size_t inFlight = 0, pos = 0;

  double start = timedouble();
  double last = start, lastsubmit =start, lastreceive = start;
  logSpeedReset(benchl);
  logSpeedReset(alll);

  size_t submitted = 0, flushPos = 0, received = 0;
  size_t totalWriteBytes = 0, totalReadBytes = 0;
  
  size_t lastBytes = 0, lastIOCount = 0;
  struct timespec timeout;
  timeout.tv_sec = 0;
  timeout.tv_nsec = 100*1000; // 0.0001 seconds

  double flush_totaltime = 0, flush_mintime = 9e99, flush_maxtime = 0;
  size_t flush_count = 0;
  double thistime = 0;
  int qdIndex = 0;
  while (keepRunning && ((thistime = timedouble()) < finishtime)) {
    if (inFlight < QD) {
      
      // submit requests, one at a time
      //      fprintf(stderr,"max %zd\n", MAX(QD - inFlight, 1));
      int submitCycles = MAX(QD - inFlight, 1);
      if (flushEvery) {
	if (flushEvery < submitCycles) {
	  submitCycles = flushEvery;
	}
      }
      for (size_t i = 0; i < submitCycles; i++) {
	if (sz) {
	  if (positions[pos].action != 'S') { // if we have some positions, sz > 0
	    long long newpos = positions[pos].pos;
	    const size_t len = positions[pos].len;

	    int read = (positions[pos].action == 'R');

	    qdIndex = freeQueue[headOfQueue++]; if (headOfQueue >= QD) headOfQueue = 0;
	    assert(stillInFlight[qdIndex] == 0);

	    // setup the request
	    if (fd >= 0) {
	      if (read) {
		if (verbose >= 2) {fprintf(stderr,"[%zd] read ", pos);}
		io_prep_pread(cbs[qdIndex], fd, dataread[qdIndex], len, newpos);

		const long offset = dataread[qdIndex] - dataread[0];
		assert(offset < randomBufferSize * QD);
		pointtoposread[offset] = qdIndex; // from memory offset to qdIndex
		
		posInFlight[qdIndex] = pos; // from qdIndex back to the position array
		stillInFlight[qdIndex] = 1;
		//	        fprintf(stderr,"pos %zd, index %d, offset %ld\n", submitted, qdIndex, index);

		totalReadBytes += len;
	      } else {
		if (verbose >= 2) {fprintf(stderr,"[%zd] write ", pos);}
		io_prep_pwrite(cbs[qdIndex], fd, data[qdIndex], len, newpos);


		const long offset = data[qdIndex] - data[0];
		assert(offset < randomBufferSize * QD);
		pointtopos[offset] = qdIndex; // from memory offset to qdIndex
		
		posInFlight[qdIndex] = pos; // from qdIndex back to the position array
		stillInFlight[qdIndex] = 1;
		
		totalWriteBytes += len;
		flushPos++;
	      }
	      
	      ret = io_submit(ioc, 1, &cbs[qdIndex]);
	      thistime = timedouble();
	      positions[pos].submittime = thistime;

	      if (ret > 0) {
		inFlight++;
		lastsubmit = thistime; // last good submit
		submitted++;
		if (verbose >= 2 || (newpos & (alignment-1))) {
		  fprintf(stderr,"fd %d, pos %lld (%s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", fd, newpos, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
		}
	      
	      } else {
		fprintf(stderr,"io_submit() failed, ret = %d\n", ret); perror("io_submit()"); if(!dontExitOnErrors) abort();
	      }
	    }
	  }
	  // onto the next one
	  pos++;
	  if (pos >= sz) {
	    if (oneShot) {
	      //	      	      fprintf(stderr,"end of function one shot\n");
	      goto endoffunction; // only go through once
	    }
	    pos = 0; // don't go over the end of the array
	  }
	}
	
	double timeelapsed = thistime - last;
	if (timeelapsed >= DISPLAYEVERY) {
	  const double speed = 1.0*(totalReadBytes + totalWriteBytes - lastBytes) / timeelapsed / 1024.0 / 1024;
	  const double IOspeed = 1.0*(received - lastIOCount) / timeelapsed;
	  if (benchl) logSpeedAdd2(benchl, TOMiB(totalReadBytes + totalWriteBytes - lastBytes), (received - lastIOCount));
	  if (!tableMode) {
	    if (verbose != -1) {
	      //	      fprintf(stderr,"[%.1lf] %.1lf GiB, qd: %zd, op: %zd, [%zd], %.0lf IO/s, %.1lf MiB/s\n", gt - start, TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, submitted / (gt - start), speed);
	      fprintf(stderr,"[%.1lf] %.1lf GiB, qd: %zd, op: %zd, [%zd], %.0lf IO/s, %.1lf MiB/s\n", thistime - start, TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, IOspeed, speed);
	    }
	    if (verbose >= 2) {
	      if (flush_count) fprintf(stderr,"*info* avg flush time %.4lf (min %.4lf, max %.4lf)\n", flush_totaltime / flush_count, flush_mintime, flush_maxtime);
	    }
	  }
	  lastBytes = totalReadBytes + totalWriteBytes;
	  lastIOCount = received;
	  last = thistime;
	  //	  if ((!keepRunning) || (gt >= finishtime)) {
	  //	    fprintf(stderr,"timeout %lf ... %lf\n", gt, finishtime);
	  //	    goto endoffunction;
	  //	  }
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
	  //	  io_prep_fsync(cbs[qdIndex], fd);
	  fsync(fd);
	  double elapsed_f = timedouble() - start_f;

	  flush_totaltime += (elapsed_f);
	  flush_count++;
	  if (elapsed_f < flush_mintime) flush_mintime = elapsed_f;
	  if (elapsed_f > flush_maxtime) flush_maxtime = elapsed_f;
	}
      }
    }

    if (inFlight < 5) { // then don't timeout
      ret = io_getevents(ioc, 1, QD, events, NULL);
    } else {
      ret = io_getevents(ioc, 1, QD, events, &timeout);
    }
    lastreceive = timedouble(); // last good receive

    if (ret > 0) {
      // verify it's all ok
      int printed = 0;
      for (int j = 0; j < ret; j++) {
	//	struct iocb *my_iocb = events[j].obj;
	if (alll) logSpeedAdd2(alll, TOMiB(events[j].res), 1);

	
	int rescode = events[j].res;
	int rescode2 = events[j].res2;

	if ((rescode <= 0) || (rescode2 != 0)) { // if return of bytes written or read
	  if (!printed) {
	    fprintf(stderr,"*error* AIO failure codes: res=%d (%s) and res2=%d (%s)\n", rescode, strerror(-rescode), rescode2, strerror(-rescode2));
	    fprintf(stderr,"*error* last successful submission was %.3lf seconds ago\n", timedouble() - lastsubmit);
	    fprintf(stderr,"*error* last successful receive was %.3lf seconds ago\n", timedouble() - lastreceive);
	  }
	  printed = 1;
	  //	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
	} else {

	  // all ok
	  // calc the queue position of the result
	  // then map the queue position back to the position
	  struct iocb *my_iocb = events[j].obj;
	  long offset = (char*)my_iocb->u.c.buf - (char*)(dataread[0]);
	  size_t requestpos, qd_indx = 0;
	  if (offset>=0 && offset < randomBufferSize * QD) {
	    // read range
	    qd_indx = pointtoposread[offset];
	  } else {
	    // write range
	    offset = (char*)my_iocb->u.c.buf - (char*)(data[0]);
	    if (offset >= 0 && offset < randomBufferSize * QD) {
	      qd_indx = pointtopos[offset];
	    } else {
	      // maybe a flush
	      fprintf(stderr,"flish\n");
	      abort();

	    }
	  }
	  requestpos = posInFlight[qd_indx];
	  //	  fprintf(stderr,"returned offset %ld, %ld, original request pos %zd\n", offset, pointtopos[offset], requestpos);
	  stillInFlight[qd_indx] = 0;
	  freeQueue[tailOfQueue++] = qd_indx; if (tailOfQueue >= QD) tailOfQueue = 0;
	  positions[requestpos].finishtime = lastreceive;
	  positions[requestpos].success = 1; // the action has completed
	}
      }
      inFlight -= ret;
      received += ret;
    }
    if (ret < 0) {
      fprintf(stderr,"eek\n");
      //    perror("io_destroy");
      break;
    }

  }

  
 endoffunction:
  // receive outstanding I/Os
  while (inFlight) {
    if (inFlight) {
      if (verbose >= 1) {
	fprintf(stderr,"*info* inflight = %zd\n", inFlight);
      }
      int ret = io_getevents(ioc, inFlight, inFlight, events, NULL);
      if (ret > 0) {
	for (int j = 0; j < ret; j++) {
	  // TODO refactor into the same code as above
	  struct iocb *my_iocb = events[j].obj;
	  long offset = (char*)my_iocb->u.c.buf - (char*)(dataread[0]);
	  size_t requestpos, qd_indx;
	  if (offset>=0 && offset < randomBufferSize * QD) {
	    // read range
	    qd_indx = pointtoposread[offset];
	  } else {
	    // write range
	    offset = (char*)my_iocb->u.c.buf - (char*)(data[0]);
	    qd_indx = pointtopos[offset];
	  }
	  requestpos = posInFlight[qd_indx];
	  stillInFlight[qd_indx] = 0;
	  freeQueue[tailOfQueue++] = qd_indx; if (tailOfQueue >= QD) tailOfQueue = 0;
	  positions[requestpos].finishtime = lastreceive;
	  positions[requestpos].success = 1; // the action has completed
	}
	inFlight -= ret;
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
  free(pointtopos);
  free(pointtoposread);
  free(posInFlight);
  free(stillInFlight);
  free(freeQueue);
  io_destroy(ioc);
  

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
		    const char *randomBuffer,
		    const int fd) {


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
	
	if (lseek(fd, pos, SEEK_SET) == -1) {
	  if (errors + ioerrors < 10) {
	    fprintf(stderr,"*error* seeking to pos %zd: ", pos);
	    //	    perror("cannot seek");
	  }
	}
	//	buffer[0] = 256 - randomBuffer[0] ^ 0xff; // make sure the first char is different
	int bytesRead = read(fd, buffer, len);
	if (lseek(fd, pos, SEEK_SET) == -1) {
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



