#define _GNU_SOURCE

#include <stdlib.h>
#include <malloc.h>
#include <termios.h>
#include <unistd.h>
#include <fcntl.h>
#include <libaio.h>
#include <assert.h>
#include <math.h>

#include "utils.h"
#include "logSpeed.h"
#include "aioRequests.h"

extern volatile int keepRunning;

#define DISPLAYEVERY 1

void checkArray(const unsigned short *freeQueue, const size_t QD) {
  if (0) { // debug print
    fprintf(stderr,"Sub: ");
    for (size_t q1 = 0; q1 < QD; q1++) {
      if (freeQueue[q1] >= 0 && freeQueue[q1] < QD) {
	fprintf(stderr,"%3u", freeQueue[q1]);
      } else {
	fprintf(stderr,"  .");
      }
    }
    fprintf(stderr,"\n");
  }
  // check for incorrect duplications
  for (size_t q1 = 0 ; q1 < QD; q1++) if (freeQueue[q1] >=0 && freeQueue[q1] < QD) {
      for (size_t q2 = 0; q2 < QD; q2++) if (q1!=q2) {
	  if (freeQueue[q1] == freeQueue[q2]) fprintf(stderr,"duplicate %u\n", freeQueue[q1]);
	  assert (freeQueue[q1] != freeQueue[q2]);
	}
    }
}

size_t aioMultiplePositions( positionContainer *p,
			     const size_t sz,
			     const double finishTime,
			     const size_t finishBytes,
			     size_t origQD,
			     const int verbose,
			     const int tableMode, 
			     logSpeedType *alll,
			     logSpeedType *benchl,
			     const char *randomBuffer,
			     const size_t randomBufferSize,
			     size_t alignment,
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
  if (origQD >= sz)  {
    origQD = sz;
    fprintf(stderr,"*info* QD reduced due to limited positions. Setting q=%zd (verbose %d)\n", origQD, verbose);
    //    exit(1);
  }
  assert(origQD <= sz);
  const size_t QD = origQD;
  assert(sz>0);
  positionType *positions = p->positions;

  //  const double alignbits = log(alignment)/log(2);
  //  assert (alignbits == (size_t)alignbits);

  io_context_t ioc = 0;
  if (io_setup(QD, &ioc)) {
    fprintf(stderr,"*error* io_setup failed with %zd\n", QD);
    exit(-2);
  }

  assert(QD);
  if (!alignment) alignment=512;
  assert(alignment);

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
  
  /* setup I/O control block, randomised just for this run. So we can check verification afterwards */
  char **data = NULL;
  CALLOC(data, QD, sizeof(char*));

  // setup the buffers to be contiguous
  if (randomBufferSize * QD >= totalRAM()) {
    fprintf(stderr,"*info* can't allocate (block size %zd x QD %zd) bytes\n", randomBufferSize, QD);
    exit(-1);
  }
  //  if (verbose) fprintf(stderr,"*info* allocating %zd bytes\n", randomBufferSize * QD);
  CALLOC(data[0], randomBufferSize * QD, 1);

  char **readdata = NULL;
  CALLOC(readdata, QD, sizeof(char*));

  // setup the buffers to be contiguous
  //  if (verbose) fprintf(stderr,"*info* allocating %zd bytes\n", randomBufferSize * QD);
  CALLOC(readdata[0], randomBufferSize * QD, 1);

  unsigned short *freeQueue; // qd collisions
  size_t headOfQueue = 0, tailOfQueue = 0;
  CALLOC(freeQueue, QD, sizeof(size_t));
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
    readdata[i] = readdata[0] + (randomBufferSize * i);
  }

  // copy the randomBuffer to each data[]
  for (size_t i = 0; i < QD; i++) {
    if (verbose >= 2) {
      fprintf(stderr,"randomBuffer[%zd]: %p\n", i, (void*)data[i]);
    }
    strncpy(data[i], randomBuffer, randomBufferSize);
  }

  size_t inFlight = 0, pos = 0;

  double start = timedouble();
  double last = start;
  //  double lastreceive = start;

  size_t submitted = 0, flushPos = 0, received = 0;
  size_t totalWriteBytes = 0, totalReadBytes = 0;
  
  size_t lastBytes = 0, lastIOCount = 0;

  struct timespec timeout;
  timeout.tv_sec = 0;
  timeout.tv_nsec = 10000*1000; // 1ms seconds

  double flush_totaltime = 0, flush_mintime = 9e99, flush_maxtime = 0;
  size_t flush_count = 0;
  double thistime = 0;
  int qdIndex = 0;
  int printed = 0;


  for (size_t i = 0; i < sz; i++) {
    positions[i].inFlight = 0;
    positions[i].success = 0;
  }

  if (verbose >= 2)fprintf(stderr,"*info* starting...%zd, finishTime %lf\n", sz, finishTime);
  while (keepRunning && ((thistime = timedouble()) < finishTime)) {
    assert (pos < sz);
    if (0) fprintf(stderr,"pos %zd, inflight %zd (%zd %zd)\n", positions[pos].pos, inFlight, tailOfQueue, headOfQueue);
    if (inFlight > QD) {
      fprintf(stderr,"*error* inFlight %zd %zd\n", inFlight, QD);
    }
	thistime = timedouble();
    while (sz && inFlight < QD && keepRunning) {
      if (!positions[pos].inFlight) {
      
      // submit requests, one at a time
	assert(pos < sz);
	if (positions[pos].action != 'S') { // if we have some positions, sz > 0
	  const size_t newpos = positions[pos].pos;
	  const size_t len = positions[pos].len;

	  assert(headOfQueue < QD);
	  qdIndex = freeQueue[headOfQueue];
	  assert(qdIndex >= 0);
	  assert(qdIndex < QD);

	  assert(positions[pos].inFlight == 0);

	  // setup the request
	  if (fd >= 0) {
	    positions[pos].q = qdIndex;
	    positions[pos].inFlight = 1;

	    // watermark the block with the position on the device
	    
	    if (positions[pos].action=='R') {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] read qdIndex=%d\n", newpos, qdIndex);}

	      io_prep_pread(cbs[qdIndex], fd, readdata[qdIndex], len, newpos);
	      cbs[qdIndex]->data = &positions[pos];
	    } else if (positions[pos].action=='F') {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] flush qdIndex=%d\n", newpos, qdIndex);}

	      io_prep_fsync(cbs[qdIndex], fd);
	      cbs[qdIndex]->data = &positions[pos];
	    } else if (positions[pos].action == 'W') {
	      if (verbose >= 2) {fprintf(stderr,"[%zd] write qdIndex=%d\n", newpos, qdIndex);}

	      size_t *posdest = (size_t*)data[qdIndex];
	      *posdest = newpos;

	      size_t *uuiddest = (size_t*)data[qdIndex] + 1;
	      *uuiddest = p->UUID;

	      io_prep_pwrite(cbs[qdIndex], fd, data[qdIndex], len, newpos);
	      cbs[qdIndex]->data = &positions[pos];

	      flushPos++;
	    }
	    
	    positions[pos].submitTime = thistime;
	      
	    ret = io_submit(ioc, 1, &cbs[qdIndex]);
	      
	    if (ret > 0) {
	      // if success
	      freeQueue[headOfQueue] = -1; // take off queue
	      //	      freeQueue[headOfQueue] = -1;
	      headOfQueue++; if (headOfQueue == QD) headOfQueue = 0;

	      inFlight++;
	      //	      lastsubmit = thistime; // last good submit
	      submitted++;
	      if (verbose >= 2 || (newpos & (alignment - 1))) {
		fprintf(stderr,"fd %d, pos %zd (%% %zd = %zd ... %s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", fd, newpos, alignment, newpos % alignment, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
	      }
	      
	    } else {
	      fprintf(stderr,"io_submit() failed, ret = %d\n", ret); perror("io_submit()"); if(!dontExitOnErrors) abort();
	    }
	  }
	}
      } else {
	if (verbose >= 1) {
	  fprintf(stderr,"*info* position collision %zd\n",pos);
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
    } // while not enough inflight
    
    double timeelapsed = timedouble() - last;
    if (timeelapsed >= DISPLAYEVERY) {
      const double speed = TOMB(1.0*(totalReadBytes + totalWriteBytes - lastBytes) / timeelapsed);
      const double IOspeed = 1.0*(received - lastIOCount) / timeelapsed;
      //if (benchl) logSpeedAdd2(benchl, TOMB(totalReadBytes + totalWriteBytes - lastBytes), (received - lastIOCount));
      if (!tableMode) {
	if (verbose != -1) {
	  //	      fprintf(stderr,"[%.1lf] %.1lf GiB, qd: %zd, op: %zd, [%zd], %.0lf IO/s, %.1lf MB/s\n", gt - start, TOGiB(totalReadBytes + totalWriteBytes), inFlight, received, pos, submitted / (gt - start), speed);
	  fprintf(stderr,"[%.1lf] %.1lf GB, qd: %zd, op: %zd, [%zd], %.0lf IO/s, %.1lf MB/s\n", thistime - start, TOGB(totalReadBytes + totalWriteBytes), inFlight, received, pos, IOspeed, speed);
	}
	if (verbose >= 2) {
	  if (flush_count) fprintf(stderr,"*info* avg flush time %.4lf (min %.4lf, max %.4lf)\n", flush_totaltime / flush_count, flush_mintime, flush_maxtime);
	}
      }
      lastBytes = totalReadBytes + totalWriteBytes;
      lastIOCount = received;
      last = thistime;
    }
  
    if (flushEvery) {
      if (flushPos >= flushEvery) {
	flushPos = flushPos - flushEvery;
	if (verbose >= 2) {
	  fprintf(stderr,"[%zd] SYNC: calling fsync()\n", pos);
	}
	fsync(fd);
	flush_count++;
      }
    }

    // return, 1..inFlight wait for a bit
    ret = io_getevents(ioc, 1, inFlight, events, &timeout);
    //    }
    if (ret > 0) {
      //      lastreceive = timedouble();

      // verify it's all ok
      size_t rio = 0, rlen = 0, wio = 0, wlen = 0;
      for (int j = 0; j < ret; j++) {
	//	struct iocb *my_iocb = events[j].obj;
	//if (alll) logSpeedAdd2(alll, TOMB(events[j].res), 1);
	struct iocb *my_iocb = events[j].obj;
	positionType *pp = (positionType*) my_iocb->data;
	assert(pp->inFlight);

	
	int rescode = events[j].res;
	int rescode2 = events[j].res2;

	if ((rescode < 0) || (rescode2 != 0)) { // if return of bytes written or read
	  if (printed++ < 10) {
	    fprintf(stderr,"*error* AIO failure codes: res=%d and res2=%d, [%zd] = %zd, inFlight %zd, returned %d results\n", rescode, rescode2, pos, positions[pos].pos, inFlight, ret);
	    //	    fprintf(stderr,"*error* last successful submission was %.3lf seconds ago\n", timedouble() - lastsubmit);
	    //	    fprintf(stderr,"*error* last successful receive was %.3lf seconds ago\n", timedouble() - lastreceive);
	  } else {
	    fprintf(stderr,"*error* further output supressed\n");
	  }
	  //	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
	} else {
	  //	  fprintf(stderr,"---> %d %d\n", rescode, rescode2);
	  //successful result

	  //	  if (pp->success) {
	  //	    fprintf(stderr,"*warning* AIO duplicate result at position %zd\n", pp->pos);
	  //	  }
	  
	  //fprintf(stderr,"'%c' %d %u\n", pp->action, pp->q, pp->len);
	  if (pp->action == 'R') {
	    rio++;
	    rlen += pp->len;
	  } else if (pp->action == 'W') {
	    wio++;
	    wlen += pp->len;
	  }

	  // if we know we have written we can check, or if we have read a previous write
	  size_t *uucheck , *poscheck;
	  if (pp->action == 'W') {
	    poscheck = (size_t*)data[pp->q];
	    uucheck = (size_t*)data[pp->q] + 1;
	    } else {
	    poscheck = (size_t*)readdata[pp->q];
	    uucheck = (size_t*)readdata[pp->q] + 1;
	  }
	  
	  if ((pp->action == 'W') || (pp->verify)) {
	    //	    if (pp->verify) fprintf(stderr,"checking..\n");
	    if ((p->UUID != *uucheck) || (pp->pos != *poscheck)) {
	      fprintf(stderr,"*error* position (success %d) %zd ver=%d wrong. UUID %zd/%zd, pos %zd/%zd\n", pp->success, pp->pos, pp->verify, p->UUID, *uucheck, pp->pos, *poscheck);
	      fprintf(stderr,"*error* combinations of meta-data 'm', multiple threads 'j' and without G_ will fail\n");
	      fprintf(stderr,"*error* ... as the different threads will clobber data from other threads in real time\n");
	      fprintf(stderr,"*error* Potentially write to -P positions.txt and check after data is written\n");
	      //	      abort();
	    }
	  }
	} // else if no error
	pp->finishTime = timedouble();
	pp->success = 1; // the action has completed
	pp->inFlight = 0;
	//checkArray(freeQueue, QD);
	freeQueue[tailOfQueue++] = pp->q; if (tailOfQueue == QD) tailOfQueue = 0;
      } // for j

      p->readIOs += rio;
      p->readBytes += rlen;
      totalReadBytes += rlen;

      p->writtenIOs += wio;
      p->writtenBytes += wlen;
      totalWriteBytes += wlen;



      if (0) {
	checkArray(freeQueue, QD);
      }

      inFlight -= ret;
      received += ret;
    }
    
    if (finishBytes && (totalWriteBytes + totalReadBytes >= finishBytes)) {
      //      fprintf(stderr,"....%zd %zd\n", finishBytes, totalWriteBytes + totalReadBytes);
      break;
    }
      
    
    //    if (ret < 0) {
      //      fprintf(stderr,"eek\n");
    //    }
  } // while keepRunning

 endoffunction:
  // receive outstanding I/Os

  {}
  size_t count = 0;
  double snaptime = timedouble();
  double lastprint = snaptime;
  while (inFlight) {
    count++;
    if (count > 3600) break;
    
    if (inFlight) {
      int ret = io_getevents(ioc, 0, inFlight, events, NULL);
      if (ret > 0) {
	for (int j = 0; j < ret; j++) {
	  // TODO refactor into the same code as above
	  struct iocb *my_iocb = events[j].obj;
	  positionType *pp = (positionType*) my_iocb->data;

	  freeQueue[tailOfQueue++] = pp->q; if (tailOfQueue == QD) tailOfQueue = 0;
	  
	  pp->finishTime = timedouble();
	  pp->inFlight = 0;
	  pp->success = 1; // the action has completed

	  if (pp->action == 'R') {
	    p->readIOs++;
	    p->readBytes += pp->len;
	    totalReadBytes += pp->len;
	  } else if (pp->action == 'W') {
	    p->writtenIOs++;
	    p->writtenBytes += pp->len;
	    totalWriteBytes += pp->len;
	  }
	}
	inFlight -= ret;
      } else {
	if (count > 5 && (timedouble() - lastprint >=3)) {
	  fprintf(stderr,"*warning* waiting for %zd IOs in flight, iteration %zd, %zd seconds...\n", inFlight, count, (size_t)(timedouble() - snaptime));
	  lastprint = timedouble();
	}
	usleep(10000);
      }
    }
  }
  if (inFlight) {
    fprintf(stderr,"*warning* inFlight requests still = %zd\n", inFlight);
  }
  
  free(events);
  for (size_t i = 0; i < QD; i++) {
    free(cbs[i]);
  }
  free(cbs);

  free(data[0]);
  free(data);
  free(readdata[0]);
  free(readdata);
  free(freeQueue);
  if (inFlight) {
    fprintf(stderr,"*warning* about to io_destroy()\n");
  }
  io_destroy(ioc);
  if (inFlight) {
    fprintf(stderr,"*info* io_destroy() succeeded\n");
  }
  
  *ios = received;

  *totalWB = totalWriteBytes;
  *totalRB = totalReadBytes;

  for (size_t i = 0; i < pos; i++) {
    assert(positions[i].submitTime > 0);
  }
  
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
  CALLOC(buffer, maxBufferSize, sizeof(char));

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
  fprintf(stderr,"*info* started verification (%zd positions, %.1lf GiB)\n", posTOV, TOGB(bytesToVerify));

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
	assert(len >= 0);
	
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
  fprintf(stderr,"checked %zd/%zd blocks, I/O errors %zd, errors/incorrect %zd, elapsed = %.1lf secs (%.1lf MB/s)\n", checked, posTOV, ioerrors, errors, elapsed, TOMB(bytesToVerify)/elapsed);


  free(buffer);

  return ioerrors + errors;
}



