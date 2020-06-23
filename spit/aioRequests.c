#define _GNU_SOURCE

#include <stdlib.h>
#include <malloc.h>
#include <termios.h>
#include <unistd.h>
#include <fcntl.h>
#include <libaio.h>
#include <assert.h>
#include <math.h>
#include <time.h>

#include "utils.h"
#include "logSpeed.h"
#include "aioRequests.h"

extern volatile int keepRunning;

#define DISPLAYEVERY 1

size_t aioMultiplePositions( positionContainer *p,
                             const size_t sz,
                             const double finishTime,
                             const size_t finishBytes,
                             size_t origQD,
                             const int verbose,
                             const int tableMode,
                             size_t alignment,
                             size_t *ios,
                             size_t *totalRB,
                             size_t *totalWB,
                             const size_t oneShot,
                             const int dontExitOnErrors,
                             const int fd,
                             int flushEvery,
                             const size_t targetMBps,
                             size_t *ioerrors,
                             size_t QDbarrier,
			     const size_t discard_max_bytes
                           )
{
  if (sz == 0) {
    fprintf(stderr,"*warning* sz == 0!\n");
    return 0;
  }
  int ret, checkTime = finishTime > 0;
  
  //  fprintf(stderr,"*this time %lf set %lf\n", timedouble(), finishTime);

  if ((finishTime > 0) && (finishTime < timedouble())) {
    //    fprintf(stderr,"*warning* ignoring time as it's set in the past\n");
    checkTime = 0;
  }
  
  struct iocb **cbs;
  struct io_event *events;
  if (origQD >= sz)  {
    origQD = sz;
    //    fprintf(stderr,"*info* QD reduced due to limited positions. Setting q=%zd (verbose %d)\n", origQD, verbose);
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

  size_t maxSize = 0;
  for (size_t i = 0; i < sz; i++) {
    if (positions[i].len > maxSize) {
      maxSize = positions[i].len;
    }
  }
  assert(maxSize > 0);

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
  for (size_t i = 0; i < QD; i++) {
    generateRandomBuffer(data[i], maxSize, firstseed);
    generateRandomBuffer(readdata[i], maxSize, firstseed);
    dataseed[i] = firstseed;
  }

  size_t inFlight = 0, pos = 0;

  double start = timedouble();
  double last = start;
  //  double lastreceive = start;

  size_t submitted = 0, flushPos = 0, received = 0;
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


  for (size_t i = 0; i < sz; i++) {
    positions[i].inFlight = 0;
    positions[i].success = 0;
  }

  double timesincereset = timedouble();
  size_t timesinceMB = 0;
  double timesleep = 500000;


  if (verbose >= 2)fprintf(stderr,"*info* starting...%zd, finishTime %lf\n", sz, finishTime);

  size_t thiskeeprunning = 1;
  while (keepRunning && thiskeeprunning) {
    thistime = timedouble();
    if (checkTime && (thistime > finishTime)) {
      thiskeeprunning = 0;
      goto endoffunction;
      //      break;
    }
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

	  if (positions[pos].action == 'T') {
	    if (discard_max_bytes >= alignment) {
	      if (verbose >= 2) fprintf(stderr,"*info* trim at %zd len = %zd\n", newpos, len);
	      positions[pos].submitTime = timedouble();
	      performDiscard(fd, NULL, newpos, newpos+len, maxSize, alignment, NULL, 0);
	      p->writtenIOs++;
	      positions[pos].finishTime = timedouble();
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
            positions[pos].inFlight = 1;

            // watermark the block with the position on the device

            if (positions[pos].action=='R') {
              if (verbose >= 2) {
                fprintf(stderr,"[%zd] read qdIndex=%d\n", newpos, qdIndex);
              }

              /*
              if (positions[pos].seed != dataseed[qdIndex]) {
              generateRandomBuffer(readdata[qdIndex], positions[pos].len, positions[pos].seed);
              dataseed[qdIndex] = positions[pos].seed;
              }*/

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
            } else {
	      fprintf(stderr,"unknown action %c\n", positions[pos].action);
	      abort();
	    }

            positions[pos].submitTime = thistime;
            positions[pos].finishTime = 0;


            // for the speed limiting
            timesinceMB += len;

            ret = io_submit(ioc, 1, &cbs[qdIndex]);

            if (ret > 0) {
              // if success
              freeQueue[headOfQueue] = -1; // take off queue
              //	      freeQueue[headOfQueue] = -1;
              headOfQueue++;
              if (headOfQueue == QD) headOfQueue = 0;

              inFlight++;
              //	      lastsubmit = thistime; // last good submit
              submitted++;
              if (verbose >= 2 || (newpos & (alignment - 1))) {
                fprintf(stderr,"fd %d, pos %zd (%% %zd = %zd ... %s), size %zd, inFlight %zd, QD %zd, submitted %zd, received %zd\n", fd, newpos, alignment, newpos % alignment, (newpos % alignment) ? "NO!!" : "aligned", len, inFlight, QD, submitted, received);
              }

            } else {
              *ioerrors = (*ioerrors) + 1;
              fprintf(stderr,"io_submit() failed, ret = %d\n", ret);
              perror("io_submit()");
              if(!dontExitOnErrors) abort();
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
      pos++;
      if (pos >= sz) {
        if (oneShot) {
          //	      	      fprintf(stderr,"end of function one shot\n");
	  //          goto endoffunction; // only go through once
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
      //      if (flushPos >= flushEvery) {
      flushPos = flushPos - flushEvery;
      if (verbose >= 2) {
        fprintf(stderr,"[%zd] SYNC: calling fsync()\n", pos);
      }
      fsync(fd);
      flush_count++;
      //      }
    }


    /// speed limiting
    if (targetMBps) {
      double tttime = timedouble();
      double resettime = tttime - timesincereset;
      size_t speed = (timesinceMB / 1024.0 / 1024.0) / resettime;
      if (resettime > 0.1) {
        if (speed > targetMBps) {
          if (timesleep == 0) timesleep = 1;

          timesleep = timesleep * 1.02;

          timesincereset = tttime;
          timesinceMB = 0;

        } else if (speed < targetMBps) {

          timesleep = timesleep / 1.02;
          if (timesleep < 0) timesleep = 0;

          timesincereset = tttime;
          timesinceMB = 0;
        } else {
          timesincereset = tttime;
          timesinceMB = 0;
        }
        //	fprintf(stderr,"*info* sleep %.3lf, speed %zd, target %zd\n", timesleep, speed, targetMBps);
      }
      struct timespec nanslp = {0,timesleep};
      nanosleep(&nanslp, NULL);
    }


    // return, 1..inFlight wait for a bit
    if (QDbarrier) {
      if (inFlight >= QD) {
        ret = io_getevents(ioc, QD, inFlight, events, &timeout);
      } else {
        ret = 0;
      }
    } else {
      ret = io_getevents(ioc, 1, inFlight, events, &timeout);
    }

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
          *ioerrors = (*ioerrors) + 1;
          if (printed++ < 10) {
            fprintf(stderr,"*error* AIO failure codes: res=%d and res2=%d, [%zd] = %zd, inFlight %zd, returned %d results\n", rescode, rescode2, pos, positions[pos].pos, inFlight, ret);
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


          if (pp->verify && (pp->action == 'R')) {
            //	    if (pp->seed != dataseed[pp->q]) {
            //	    //n	      generateRandomBuffer(readdata[pp->q], pp->len, pp->seed);
            //	      dataseed[pp->q] = pp->seed;
            //	    }

            // if we know we have written we can check, or if we have read a previous write
            size_t *uucheck = NULL, *poscheck = NULL;
            poscheck = (size_t*)readdata[pp->q];
            uucheck = (size_t*)readdata[pp->q] + 1;

            if (((p->UUID != *uucheck) || (pp->pos != *poscheck)) && (positions[pp->verify].finishTime)) {
              fprintf(stderr,"*error* position[%zd] '%c' R=%d (success %d) ver=%d wrong. UUID %zd/%zd, pos %zd/%zd\n", pos, pp->action, pp->seed, pp->success, pp->verify, p->UUID, *uucheck, pp->pos, *poscheck);
              fprintf(stderr,"*error* Maybe: combinations of meta-data 'm', multiple threads 'j' and without G_ may fail\n");
              fprintf(stderr,"*error* as the different threads will clobber data from other threads in real time\n");
              fprintf(stderr,"*error* Potentially write to -P positions.txt and check after data is written\n");
              abort();
            }
          }
        } // else if no error
        pp->finishTime = timedouble();
        pp->success = 1; // the action has completed
        pp->inFlight = 0;
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

          freeQueue[tailOfQueue++] = pp->q;
          if (tailOfQueue == QD) tailOfQueue = 0;

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
    fprintf(stderr,"*warning* about to io_destroy()... should be instant before a 'succeeded' mvessage.\n");
  }
  io_destroy(ioc);
  if (inFlight) {
    fprintf(stderr,"*info* io_destroy() succeeded\n");
  }

  *ios = received;

  *totalWB = totalWriteSubmit;
  *totalRB = totalReadSubmit;

  for (size_t i = 0; i < pos; i++) {
    if (positions[i].action == 'R' || positions[i].action == 'W') {
      assert(positions[i].submitTime > 0);
    }
  }

  return (*totalWB) + (*totalRB);
}

