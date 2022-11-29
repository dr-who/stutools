#define _GNU_SOURCE

#include <stdlib.h>
#include <termios.h>
#include <unistd.h>
#include <fcntl.h>
#include <libaio.h>
#include <assert.h>
#include <math.h>
#include <time.h>

#include "linux-headers.h"

#include "utils.h"
#include "logSpeed.h"
#include "aioRequests.h"
#include "positions.h"

extern volatile int keepRunning;

#define DISPLAYEVERY 1

size_t aioMultiplePositions( positionContainer *p,
                             const size_t sz,
                             const double finishTime,
                             const size_t finishBytes,
                             size_t QDmin,
                             size_t QDmax,
                             const int verbose,
                             const int tableMode,
                             size_t alignment,
                             size_t *ios,
                             size_t *totalRB,
                             size_t *totalWB,
                             const size_t rounds,
			     const size_t posStart,
                             const size_t posLimit,
			     size_t *posCompleted,
                             const int dontExitOnErrors,
                             const int fd,
                             int flushEvery,
                             size_t *ioerrors,
                             const size_t discard_max_bytes,
			     FILE *fp,
			     char *jobdevice,
			     size_t posIncrement,
			     int recordSamples,
			     size_t alternateEvery,
			     size_t stringcompare
                           )
{
  size_t printCount = 0;
  if (stringcompare) {
    fprintf(stderr,"*info* strong byte comparison enabled.\n");
  }

  if (sz == 0) {
    fprintf(stderr,"*warning* sz == 0!\n");
    return 0;
  }

  //  fprintf(stderr,"aio %zd\n", p->UUID);
  if (posIncrement > 1) {
    fprintf(stderr,"*info* positions %zd, rounds %zd, posIncrement %zd\n", posLimit, rounds, posIncrement);
  }

  if (recordSamples) {
    fprintf(stderr,"*info* allocating latencies for %d samples at %zd positions\n", recordSamples, sz);
    for (size_t i = 0; i < sz; i++) {
      if (p->positions[i].latencies == NULL) {
	p->positions[i].latencies = malloc(recordSamples * sizeof(float));
	memset(&p->positions[i].latencies[0], 0, recordSamples * sizeof(float));
	assert(p->positions[i].latencies);
      }
    }
  }
  
  //  if (finishBytes) fprintf(stderr,"*info* maxbytes %zd\n", finishBytes);
  //  if (posLimit) fprintf(stderr,"*info* maxpositions %zd\n", posLimit);
  
  int ret, checkTime = finishTime > 0;
  if (posIncrement < 1) posIncrement = 1;
  

  //  fprintf(stderr,"*this time %lf set %lf, checkTime %d\n", timedouble(), finishTime, checkTime);
  //  if (posLimit) {
  //    if (verbose) fprintf(stderr,"*info* limit positions to %zd\n", posLimit);
    //  }
  
  if ((finishTime > 0) && (finishTime < timedouble())) {
    //    fprintf(stderr,"*warning* ignoring time as it's set in the past\n");
    checkTime = 0;
  }

  struct iocb **cbs;
  struct io_event *events;

  /*  if (QDmax >= posLimit * rounds)  {
    QDmax = posLimit ;
    fprintf(stderr,"*info* QD reduced due to limited positions. Setting q=%zd (verbose %d)\n", QDmax, verbose);
    //    exit(1);
    }*/
  if (QDmax > sz) {
    QDmax = MIN(QDmax, sz);
    fprintf(stderr,"*warning* decreasing QDmax=%zd because sz=%zd\n", QDmax, sz);
  }
  assert(QDmax <= sz);
  const size_t QD = QDmax;
  assert(sz>0);
  positionType *positions = p->positions;
  p->inFlight = 0;


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

  size_t maxSize = 0;
  for (size_t i = 0; i < sz; i++) {
    if (positions[i].len > maxSize) {
      maxSize = positions[i].len;
    }
  }
  if (maxSize == 0) {
    fprintf(stderr,"*error* max position len == 0, not executing\n");
    return 0;
  }
  assert(maxSize > 0);
  //  assert(maxSize <= 1L*1024*1024*1024); // shouldn't be more than 1GB!?

  /* setup I/O control block, randomised just for this run. So we can check verification afterwards */
  char **data = NULL;
  CALLOC(data, QD, sizeof(char*));

  unsigned short *dataseed = NULL;
  CALLOC(dataseed, QD, sizeof(unsigned short));

  // setup the buffers to be contiguous
  if (maxSize * QD >= totalRAM()) {
    fprintf(stderr,"*info* can't allocate (block size %zd x QD %zd) bytes\n", maxSize, QD);
    exit(-1);
  }
  //  if (verbose) fprintf(stderr,"*info* allocating %zd bytes\n", randomBufferSize * QD);
  CALLOC(data[0], maxSize * QD, 1);

  char **readdata = NULL;
  CALLOC(readdata, QD, sizeof(char*));

  // setup the buffers to be contiguous
  //  if (verbose) fprintf(stderr,"*info* allocating %zd bytes\n", randomBufferSize * QD);
  CALLOC(readdata[0], maxSize * QD, 1);

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

  // setup pointers
  for (size_t i = 0; i <QD; i++) {
    data[i] = data[0] + (maxSize * i);
    readdata[i] = readdata[0] + (maxSize * i);
  }

  // copy the randomBuffer to each data[]
  // sz is already > 0
  assert(sz);
  unsigned short firstseed = positions[0].seed;

  // set the first values of all the write data
  if (QD > 0) {
    generateRandomBuffer(data[0], maxSize, firstseed);
    generateRandomBuffer(readdata[0], maxSize, firstseed);
  }
    
  for (size_t i = 0; i < QD; i++) {
    if (i > 0) {
      memcpy(data[i], data[0], maxSize);
      memcpy(readdata[i], readdata[0], maxSize);
    }
    //    generateRandomBuffer(data[i], maxSize, firstseed);
    //    generateRandomBuffer(readdata[i], maxSize, firstseed);
    dataseed[i] = firstseed;
  }

  size_t inFlight = 0, pos = posStart;
  if (sz) {
    pos = pos % sz;
  }
  assert (pos < sz);
  *posCompleted = 0;

  const double start = timedouble();
  double last = start, roundstart = start;
  //  double lastreceive = start;

  size_t submitted = 0, flushPos = 0, received = 0, slow = 0;
  size_t totalWriteBytes = 0, totalReadBytes = 0;
  size_t totalWriteSubmit = 0, totalReadSubmit = 0;

  size_t lastBytes = 0, lastIOCount = 0;

  struct timespec timeout;
  timeout.tv_sec = 0;
  timeout.tv_nsec = 10000*1000; // 1ms seconds

  double flush_totaltime = 0, flush_mintime = 9e99, flush_maxtime = 0;
  size_t flush_count = 0;
  double thistime = 0;
  int qdIndex = 0;
  int printed = 0;


  if (posStart == 0) {
    // if from the start then clear, else leave alone
    for (size_t i = 0; i < sz; i++) {
      positions[i].inFlight = 0;
      positions[i].submitTime = 0;
      positions[i].finishTime = 0;
      positions[i].success = 0;
    }
  }
  

  size_t timesinceMB = 0;

  if (verbose >= 2)fprintf(stderr,"*info* starting...%zd, finishTime %lf\n", sz, finishTime);

  size_t thiskeeprunning = 1;
	
  while (keepRunning && thiskeeprunning) {
    thistime = timedouble();

    if (inFlight > QD) {
      fprintf(stderr,"*error* inFlight %zd %zd\n", inFlight, QD);
    }

    assert (pos < sz);
    if (thistime >= roundstart + positions[pos].usoffset) {
      size_t submitthisround = 0;
      while (sz && (inFlight < QDmax) && keepRunning) {

	if (checkTime && (thistime > finishTime)) {
	  thiskeeprunning = 0;
	  goto endoffunction;
	  //      break;
	}

	if (!positions[pos].inFlight) {
	  submitthisround++;
	  if (submitthisround > QDmin) break; // only submit QDmin at a time

	  // submit requests, one at a time
	  assert(pos < sz);
	  if (positions[pos].action != 'S') { // if we have some positions, sz > 0
	    const size_t newpos = positions[pos].pos;
	    const size_t len = positions[pos].len;

	    if (positions[pos].action == 'T') {
	      if (discard_max_bytes >= alignment) {
		if (verbose >= 2) fprintf(stderr,"*info* trim at %zd len = %zd\n", newpos, len);
		positions[pos].submitTime = timedouble();
		performDiscard(fd, NULL, newpos, newpos+len, maxSize, alignment, NULL, 0, 0);
		p->writtenIOs++;
		positions[pos].finishTime = timedouble();
		positions[pos].sumLatency += (positions[pos].finishTime - positions[pos].submitTime);
		if (recordSamples && positions[pos].samples < recordSamples) {
		  int pppp = positions[pos].samples++;
		  //		  positions[pos].latencies = realloc(positions[pos].latencies, (pppp+1) * sizeof(float));
		  //		  assert(positions[pos].latencies);
		  positions[pos].latencies[pppp] = positions[pos].finishTime - positions[pos].submitTime;
		}
	      }

	      goto nextpos;
	    }

	    assert(headOfQueue < QD);
	    qdIndex = freeQueue[headOfQueue];
	    assert(qdIndex >= 0);

	    assert(positions[pos].inFlight == 0);

	    // setup the request
	    if (fd >= 0) {
	      positions[pos].q = qdIndex;

	      // watermark the block with the position on the device

	      if (alternateEvery) {
		const double thepass = (submitted * 1.0 / alternateEvery);
		if (thepass >= 1) { // after the first pass start
		  // swap R and W
		  if (positions[pos].action == 'R') {
		    positions[pos].action = 'W';
		    positions[pos].verify = 0;
		  } else if (positions[pos].action == 'W') {
		    positions[pos].action = 'R';
		    positions[pos].verify = 1; // if was W now R with a verify
		  }
		}
	      }

	      if (positions[pos].action=='D') {
		abort();
		thistime = timedouble();

		//if (verbose >= 2) {
		//	      fprintf(stderr,"delay %u\n", positions[pos].msdelay * 1000);
		//	      }

		goto nextpos;
	      } else if (positions[pos].action=='R') {
		if (verbose >= 2) {
		  fprintf(stderr,"[%zd] read qdIndex=%d\n", newpos, qdIndex);
		}

		if (stringcompare) {
		  if (positions[pos].seed != dataseed[qdIndex]) {
		    //		    fprintf(stderr,"seed changed\n");
		    generateRandomBuffer(data[qdIndex], positions[pos].len, positions[pos].seed);
		    dataseed[qdIndex] = positions[pos].seed;
		  }
		}

		io_prep_pread(cbs[qdIndex], fd, readdata[qdIndex], len, newpos);
		cbs[qdIndex]->data = &positions[pos];

		if (finishBytes && (totalWriteSubmit + totalReadSubmit + len > finishBytes)) {
		  goto endoffunction;
		}
		totalReadSubmit += len;


	      } else if (positions[pos].action=='F') {
		if (verbose >= 2) {
		  fprintf(stderr,"[%zd] flush qdIndex=%d\n", newpos, qdIndex);
		}

		io_prep_fsync(cbs[qdIndex], fd);
		cbs[qdIndex]->data = &positions[pos];
	      } else if (positions[pos].action == 'W') {
		if (verbose >= 2) {
		  fprintf(stderr,"[%zd] write qdIndex=%d\n", newpos, qdIndex);
		}

		if (positions[pos].seed != dataseed[qdIndex]) {
		  //		  fprintf(stderr,"seed chnged\n");
		  generateRandomBuffer(data[qdIndex], positions[pos].len, positions[pos].seed);
		  dataseed[qdIndex] = positions[pos].seed;
		}

		size_t *posdest = (size_t*)data[qdIndex];
		*posdest = newpos;

		size_t *uuiddest = (size_t*)data[qdIndex] + 1;
		*uuiddest = p->UUID;

		if (positions[pos].verify) {
		  if (positions[positions[pos].verify].finishTime == 0) {
		    positions[pos].verify = 0;
		  }
		}

		io_prep_pwrite(cbs[qdIndex], fd, data[qdIndex], len, newpos);
		cbs[qdIndex]->data = &positions[pos];

		if (finishBytes && (totalWriteSubmit + totalReadSubmit + len > finishBytes)) {
		  goto endoffunction;
		}
		totalWriteSubmit += len;

		flushPos++;
	      } else if (positions[pos].action == 'P')  {
		if (inFlight > 0) {
		  pos--;  // if in flight repeat
		  goto nextpos;
		} else {
		  // else skip this one
		  goto nextpos;
		}
	      } else {
		fprintf(stderr,"unknown action[%zd] '%c' (%d)\n", pos, positions[pos].action, positions[pos].action);
		//		abort();
		goto nextpos;
	      }

	      positions[pos].submitTime = timedouble();
	      positions[pos].finishTime = 0; 


	      // for the speed limiting
	      timesinceMB += len;

	      const double sub_start = positions[pos].submitTime;
	      positions[pos].inFlight = 1;
	      
	      ret = io_submit(ioc, 1, &cbs[qdIndex]);
	      const double sub_taken = timedouble() - sub_start;
	      if (sub_taken > 1) {
		fprintf(stderr,"*info* io_submit took %lf seconds (async submit, queue full?)\n", sub_taken);
	      }

	      if (ret > 0) {
		// if success
		freeQueue[headOfQueue] = -1; // take off queue
		//	      freeQueue[headOfQueue] = -1;
		headOfQueue++;
		if (headOfQueue == QD) headOfQueue = 0;

		inFlight++;
		p->inFlight = inFlight;
		//	      lastsubmit = thistime; // last good submit
		submitted++;
		if (verbose >= 2 || (newpos & (alignment - 1))) {
		  fprintf(stderr,"fd %d, pos %zd (%% %zd = %zd ... %s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", fd, newpos, alignment, newpos % alignment, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
		}

	      } else {
		*ioerrors = (*ioerrors) + 1;
		fprintf(stderr,"io_submit() failed, ret = %d\n", ret);
		perror("io_submit()");
		if(!dontExitOnErrors) exit(-1);
	      }
	    }
	  }
	} else {
	  if (verbose >= 1) {
	    fprintf(stderr,"*info* position collision %zd\n",pos);
	  }
	}



      nextpos:

	// onto the next one
	pos += posIncrement;
	if (posCompleted) (*posCompleted)++;
	if (pos >= sz) {
	  pos = 0;
	  roundstart = thistime;
	}

	if (posLimit && (submitted >= posLimit * rounds)) {
	  // if Px is passed in
	  //	  fprintf(stderr,"end of function one shot\n");
	  goto endoffunction; // only go through once
	}
      } // while not enough inflight
    }

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
      //      if (flushPos >= flushEvery) {
      flushPos = flushPos - flushEvery;
      if (verbose >= 2) {
        fprintf(stderr,"[%zd] SYNC: calling fsync()\n", pos);
      }
      fsync(fd);
      flush_count++;
      //      }
    }


    if (inFlight == QD) {
      // if max inflight pause until at least one
      ret = io_getevents(ioc, 1, inFlight, events, &timeout);
    } else {
      // fast poll
      ret = io_getevents(ioc, 0, inFlight, events, NULL);
    }

    
    if (ret > 0) {
      double lastreceive = timedouble();

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

        if ((rescode < 0) || (rescode2 != 0)) { // if return of bytes written or read / IO error
          *ioerrors = (*ioerrors) + 1;
          if (printed++ < 10) {
            fprintf(stderr,"*error* AIO failure codes[fd=%d]: res=%d and res2=%d, [%zd] = %zd, len %d, inFlight %zd, returned %d results\n", fd, rescode, rescode2, pos, positions[pos].pos, positions[pos].len, inFlight, ret);
            //	    fprintf(stderr,"*error* last successful submission was %.3lf seconds ago\n", timedouble() - lastsubmit);
            //	    fprintf(stderr,"*error* last successful receive was %.3lf seconds ago\n", timedouble() - lastreceive);
          } else {

            //	    fprintf(stderr,"*error* further output supressed\n");
          }
          if (*ioerrors > 1000000) {
            fprintf(stderr,"*info* over %zd IO errors. Exiting...\n", *ioerrors);
            exit(-1);
          }
          //	  fprintf(stderr,"%ld %s %s\n", events[j].res, strerror(events[j].res2), (char*) my_iocb->u.c.buf);
        } else { // good IO
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


          if (pp->verify && (pp->action == 'R')) {
            // if we know we have written we can check, or if we have read a previous write
            size_t *uucheck = NULL, *poscheck = NULL;
            poscheck = (size_t*)readdata[pp->q];
            uucheck = (size_t*)readdata[pp->q] + 1;

	    int strcmpres = 0, firstcmp = 0;
	    if (stringcompare) {
	      strcmpres = strncmp(readdata[pp->q] + 16, data[pp->q] + 16, pp->len-16) || (pp->pos != *poscheck);
	    } else {
	      firstcmp = /*(p->UUID != *uucheck) ||*/ (pp->pos != *poscheck);
	    }
	    
            if (((firstcmp) || (strcmpres != 0)) && (positions[pp->verify].finishTime)) {
	      
              fprintf(stderr,"*error* position[%zd] cmp=%d, '%c' R=%d (success %d) ver=%d wrong. UUID %zd/%zd, pos %zd/%zd\n", pos, strcmpres, pp->action, pp->seed, pp->success, pp->verify, p->UUID, *uucheck, pp->pos, *poscheck);
              fprintf(stderr,"*error* Maybe: combinations of meta-data 'm', multiple threads 'j' and without G_ may fail\n");
              fprintf(stderr,"*error* as the different threads will clobber data from other threads in real time\n");
              fprintf(stderr,"*error* Potentially write to -P positions.txt and check after data is written\n");
	      exit(-1);
	      //              abort();
            }
          }
	  pp->finishTime = lastreceive;
	  pp->sumLatency += (pp->finishTime - pp->submitTime);
	  if (recordSamples && pp->samples < recordSamples) {
	    int pppp = pp->samples++;
	    //	    pp->latencies = realloc(pp->latencies, (pppp+1) * sizeof(float));
	    //	    assert(pp->latencies);
	    pp->latencies[pppp] = pp->finishTime - pp->submitTime;
	  }
        } // good IO
        pp->success = 1; // the action has completed
        pp->inFlight = 0;
	if (fp == stdout) {
	  positionDumpOne(fp, pp, p->maxbdSize, 0, jobdevice, printCount++);
	  if (printCount >= sz) printCount = 0;
	}
        // log if slow
        if (pp->finishTime - pp->submitTime > 30) {
          slow++;
          char s[300];
          sprintf(s, "slow I/O (%c,pos=%zd,size=%d) %.1lf s, submission loop, %zd slow from %zd submitted (%.1lf%%)\n", pp->action, pp->pos, pp->len, pp->finishTime, slow, submitted, slow * 100.0 / (slow + submitted));
          syslogString("spit", s);
          fprintf(stderr,"*warning* %s", s);
        }

        freeQueue[tailOfQueue++] = pp->q;
        if (tailOfQueue == QD) tailOfQueue = 0;
      } // for j

      p->readIOs += rio;
      p->readBytes += rlen;
      totalReadBytes += rlen;

      p->writtenIOs += wio;
      p->writtenBytes += wlen;
      totalWriteBytes += wlen;

      inFlight -= ret;
      received += ret;
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

          int rescode = events[j].res;
          int rescode2 = events[j].res2;

          if ((rescode < 0) || (rescode2 != 0)) { // if return of bytes written or read
            *ioerrors = (*ioerrors) + 1;
    
            fprintf(stderr,"*error* AIO failure codes[fd=%d]: res=%d and res2=%d, %zd, inFlight %zd, returned %d results\n", fd, rescode, rescode2, pp->pos, inFlight, ret);

          }
	  pp->finishTime = timedouble();
	  pp->sumLatency += (pp->finishTime - pp->submitTime);
	  if (recordSamples && pp->samples < recordSamples) {
	    int pppp = pp->samples++;
	    //	    pp->latencies = realloc(pp->latencies, (pppp+1) * sizeof(float));
	    //	    assert(pp->latencies);
	    pp->latencies[pppp] = pp->finishTime - pp->submitTime;
	  }
	  
          freeQueue[tailOfQueue++] = pp->q;
          if (tailOfQueue == QD) tailOfQueue = 0;

          // log if slow
          if (pp->finishTime - pp->submitTime > 30) {
            slow++;
            char s[300];
            sprintf(s, "slow I/O (%c,pos=%zd, size=%d) %.1lf s, no submission/post loop, %zd slow from %zd submitted (%.1lf%%)\n", pp->action, pp->pos, pp->len, pp->finishTime, slow, submitted, slow * 100.0 / (slow + submitted));
            syslogString("spit", s);
            fprintf(stderr,"*warning* %s", s);
          }


          pp->inFlight = 0;
          pp->success = 1; // the action has completed
	  if (fp == stdout) {
	    positionDumpOne(fp, pp, p->maxbdSize, 0, jobdevice, printCount++);
	    if (printCount >= sz) printCount = 0;
	  }

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
    fprintf(stderr,"*warning* timed out after %.0lf seconds. Flight requests still = %zd\n", timedouble() - snaptime, inFlight);
  }

  free(events);
  for (size_t i = 0; i < QD; i++) {
    free(cbs[i]);
  }
  free(cbs);

  free(data[0]);
  free(data);
  free(dataseed);
  free(readdata[0]);
  free(readdata);
  free(freeQueue);
  if (inFlight) {
    fprintf(stderr,"*warning* about to io_destroy()... should be instant before a 'succeeded' message.\n");
  }
  io_destroy(ioc);
  if (inFlight) {
    fprintf(stderr,"*info* io_destroy() succeeded\n");
  }

  *ios = received;

  *totalWB = totalWriteSubmit;
  *totalRB = totalReadSubmit;

  for (size_t i = 0; i < sz; i++) {
    if (positions[i].submitTime > 0) {
      if (positions[i].inFlight) {
	fprintf(stderr,"*warning* position %zd inflight! submit %lf, success %d\n", positions[i].pos, positions[i].submitTime, positions[i].success);
      }
    }
  }

  for (size_t i = posStart; i < pos; i += posIncrement) {
    if (positions[i].action == 'R' || positions[i].action == 'W') {
      assert(positions[i].submitTime > 0);
    }
  }

  return (*totalWB) + (*totalRB);
}

