#define _POSIX_C_SOURCE 200809L
#define _DEFAULT_SOURCE
#define _GNU_SOURCE

/**
 * blockVerify.c
 *
 *
 */
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <assert.h>

#include "positions.h"
#include "devices.h"
#include "utils.h"
#include "blockVerify.h"
#include "jobType.h"

extern int keepRunning;
extern int verbose;

_Atomic int maxPositions = 0;
_Atomic int curPositions = 0;

typedef struct {
    jobType *job;
    int id;
    int numThreads;
    size_t startInc;
    size_t endExc;
    size_t correct;
    size_t incorrect;
    size_t ioerrors;
    size_t lenerrors;
    positionContainer *pc;
    size_t bytesRead;
    size_t iocount;
    double elapsed;
    size_t o_direct;
    size_t sorted;
    double finishTime;
    int quiet;
    int overridesize;
    size_t exitearlyafternerrors;
} threadInfoType;

// sorting function, used by qsort
static int timeCompare(const void *p1, const void *p2) {
    const positionType *pos1 = (positionType *) p1;
    const positionType *pos2 = (positionType *) p2;

    if (pos1->deviceid < pos2->deviceid) return -1;
    else if (pos1->deviceid > pos2->deviceid) return 1;
    else {
        if (pos1->submitTime < pos2->submitTime) return -1;
        else if (pos1->submitTime > pos2->submitTime) return 1;
        else return 0;
    }
}


int
verifyPosition(const int fd, const positionType *p, const char *randomBuffer, char *buf, size_t *diff, const int seed,
               int quiet, size_t overridesize) {
    const size_t pos = p->pos;
    const size_t len = overridesize ? overridesize : p->len;

    assert(p->action == 'W');
    assert(quiet || 1);
    ssize_t ret = pread(fd, buf, len, pos); // use pread as it's thread safe as you pass in the fd, size and offset

    if (ret == -1) {
        fprintTimePrefix(stderr);
        fprintf(stderr, "*error* seeking to pos %zd: ", pos);
        perror("pread");
        return -1;
    }

    int dataok = strncmp(buf + 16, randomBuffer + 16, len - 16) == 0;

    if (ret != (int) len) {
        fprintTimePrefix(stderr);
        fprintf(stderr, "*error* [%zd], wrong len %zd instead of %zd (data ok: %s)\n", pos, ret, len, dataok ? "Yes" : "No");
        return -2;
    } else { // len are the same
        size_t blank = 1;         // first check not zero
        for (size_t qq = 0; qq < len; qq++) {
	  if (buf[qq] != 0) {blank = 0; break;}
        }
	if (blank) {
	    fprintTimePrefix(stderr);
            fprintf(stderr, "[%zd] block is empty (all 0x00)\n", pos);
	    return -3;
	} else { // same len, not blank, check encoded positions
	    size_t *p1 = (size_t *) buf;
	    size_t *p2 = (size_t *) randomBuffer;
	    if (*p1 != *p2) {
	      fprintTimePrefix(stderr);
	      fprintf(stderr,
		      "*error* [%zd] encoded positions are wrong %zd (from disk) and %zd (at position %zd). Data ok: %s\n",
		      pos, *p1, *p2, pos, dataok ? "Yes" : "No");
	      return -3;
	    }
	} // not blank 

    
        if (dataok) return 0;

        if (*diff < 3) {
            size_t lines = 0, starterror = 0;
            for (size_t i = 16; i < len; i++) {
                if (buf[i] != randomBuffer[i]) {
		  if (lines < 10) {
		      fprintTimePrefix(stderr);
		      fprintf(stderr,
			    "*error* [%zd] difference at block[%zd] offset %zd, disk '%c', should be '%c', seed %d\n",
			    pos, pos, i, buf[i], randomBuffer[i], seed);
		    }
                    if (lines == 0) starterror = i;
                    lines++;
                }
            }
	    fprintTimePrefix(stderr);
            fprintf(stderr, "*error* total characters incorrect %zd from %zd\n", lines, len);

            //awk '{if((($2>=3415666688) && $2+$7<=(3415666688+122880)) || ($2+$7>=3415666688 && $2<3415666688)) print}' positions.txt

            size_t *poscheck = (size_t *) buf;
            size_t *uucheck = (size_t *) buf + 1;
	    fprintTimePrefix(stderr);
	    fprintf(stderr, "*error* poscheck at the start %zd, uuid %zd\n", *poscheck, *uucheck);
            poscheck = (size_t *) (buf + starterror);
            uucheck = (size_t *) (buf + starterror) + 1;
	    fprintTimePrefix(stderr);
            fprintf(stderr, "*error* poscheck at the error start %zd, uuid %zd\n", *poscheck, *uucheck);
            fprintf(stderr,
                    "*error* see actions e.g.: $ awk -v p=%zd -v l=%zd '{if (($2>=p && $2<=p+l) || ($2+$7>p && $2<p)) print}' positions.txt | sort -k 12n\n",
                    pos, len);
        }
        *diff = (*diff) + 1;

        return -3;
    }
}


static void *runThread(void *arg) {

    threadInfoType *threadContext = (threadInfoType *) arg; // grab the thread threadContext args

    char *buf = NULL, *randombuf = NULL;
    CALLOC(randombuf, threadContext->pc->maxbs + 1, 1);
    CALLOC(buf, threadContext->pc->maxbs + 1, 1);

    //      fprintf(stderr,"*info* id %d, [%ld, %ld),.... \n", threadContext->id, threadContext->startInc, threadContext->endExc);
    size_t lastseed = -1;
    positionType *positions = threadContext->pc->positions;
    size_t diff = 0;


    int fd = 0;
    unsigned short lastid = -1;

    size_t gap = threadContext->endExc - threadContext->startInc;
    maxPositions += gap;
    //  __atomic_fetch_add(&maxPositions, gap, __ATOMIC_SEQ_CST);


    if (threadContext->id == 0) {
        if (threadContext->sorted) {
            //if (!threadContext->quiet) fprintf(stderr,"*info* already sorted positions based on submitTime\n");
            //      qsort(&threadContext->pc->positions[threadContext->startInc], gap, sizeof(positionType), timeCompare);
        } else {
            if (!threadContext->quiet) fprintf(stderr, "*info* positions are shuffled\n");
            unsigned int seed = threadContext->id;
            for (size_t i = threadContext->startInc; i < threadContext->endExc; i++) {
                size_t j = i;
                while ((j = (threadContext->startInc + rand_r(&seed) % gap)) == i) { ;
                }
                // swap i and j
                positionType p = threadContext->pc->positions[i];
                threadContext->pc->positions[i] = threadContext->pc->positions[j];
                threadContext->pc->positions[j] = p;
            }
        }

        /* int print = 0;
        fprintf(stderr,"*info* first 10 positions in thread 0\n");
        for (size_t i = threadContext->startInc; i < threadContext->endExc; i++) {
          print++;
          fprintf(stderr,"*info* num %zd, seed %d, submitTime %lf\n", threadContext->pc->positions[i].pos, threadContext->pc->positions[i].seed, threadContext->pc->positions[i].submitTime);
          if (print >= 100) break;
          }*/
    }


    size_t quitEarly = 0;
    for (size_t i = threadContext->startInc; i < threadContext->endExc; i++) {

        //    __atomic_fetch_add(&curPositions, 1, __ATOMIC_SEQ_CST);
        curPositions++;

        if (timeAsDouble() > threadContext->finishTime) { // if reached the time limit
            quitEarly = 1;
            break;
        }

        if (threadContext->pc->positions[i].deviceid != lastid) {
            //      fprintf(stderr,"[%d] lastid %d this %d\n", threadContext->id, lastid, threadContext->pc->positions[i].deviceid);
            lastid = threadContext->pc->positions[i].deviceid;
            if (fd != 0) {
                close(fd);
            }
            if ((i == 0) && (threadContext->o_direct == 0)) {
                fprintf(stderr, "*info* turning off O_DIRECT for verification\n");
            }
            fd = open(threadContext->job->devices[threadContext->pc->positions[i].deviceid],
                      O_RDONLY | threadContext->o_direct);
            //      if (threadContext->id == 0)
            //	fprintf(stderr,"*info* opening file %s in O_RDONLY | O_DIRECT\n", threadContext->job->devices[threadContext->pc->positions[i].deviceid]);
            if (fd < 0) {
                fprintf(stderr, "*info* if the file system doesn't support O_DIRECT you may need the -D option\n");
                perror(threadContext->job->devices[threadContext->pc->positions[i].deviceid]);
                exit(-1);
            }
        }
        if (threadContext->id == threadContext->numThreads - 1) {
            // print progress
            size_t gap = threadContext->endExc - threadContext->startInc - 1;
            if (isatty(fileno(stderr))) {
                if (!threadContext->quiet) {
                    fprintf(stderr, "*progress* %.1lf %%\r", (gap == 0) ? 100 : (curPositions * 100.0) / maxPositions);
                    fflush(stderr);
                }
            }
        }
        if (!keepRunning) {
            break;
        }
        if (positions[i].action == 'W' && positions[i].finishTime > 0) {
            threadContext->bytesRead += positions[i].len;
            if (positions[i].seed != lastseed) {
                generateRandomBuffer(randombuf, threadContext->overridesize ? (size_t) threadContext->overridesize
                                                                            : threadContext->pc->maxbs,
                                     positions[i].seed);
                lastseed = positions[i].seed;
            }

            //      double start = timeAsDouble();
            size_t pos = threadContext->pc->positions[i].pos;
            memcpy(randombuf, &pos, sizeof(size_t));
            int ret = verifyPosition(fd, &threadContext->pc->positions[i], randombuf, buf, &diff, lastseed,
                                     threadContext->quiet, threadContext->overridesize);

            //      threadContext->elapsed = timeAsDouble() - start;
            switch (ret) {
                case 0:
                    threadContext->correct++;
                    break;
                case -1:
                    threadContext->ioerrors++;
                    break;
                case -2:
                    threadContext->lenerrors++;
                    break;
                case -3:
                    threadContext->incorrect++;
                    break;
                default:
                    break;
            }

            threadContext->iocount++;

            if (threadContext->exitearlyafternerrors) {
                if (threadContext->incorrect + threadContext->ioerrors + threadContext->lenerrors >
                    threadContext->exitearlyafternerrors) {
                    break;
                }
            }
        }
    }
    if ((quitEarly) && (threadContext->id == 0)) {
        fprintf(stderr, "*warning* verification terminated early due to time limit\n");
    }
    if (fd > 0) {
        close(fd);
    }
    free(buf);
    free(randombuf);

    if (verbose >= 2) {
        fprintf(stderr, "*info* verify thread %d / %d finished\n", threadContext->id + 1, threadContext->numThreads);
    }
    return NULL;
}


/**
 * Input is sorted
 *
 */
int verifyPositions(positionContainer *pc, size_t threads, jobType *job, const size_t o_direct, const size_t sorted,
                    const double runTime, size_t *r_correct, size_t *r_incorrect, size_t *r_ioerrors, int quiet,
                    int process, size_t overridesize, size_t exitafternerrorsperthread) {
    if (threads < 1) threads = 1;
    const double finishTime = (runTime <= 0) ? (timeAsDouble() + 9e99) : (timeAsDouble() + runTime);

    if (runTime > 0) {
        if (!quiet) fprintf(stderr, "*info* verification will stop after %.1lf seconds\n", runTime);
    }

    //  for (size_t i = 0; i < pc->sz; i++) {
    //    fprintf(stderr,"[%zd] pos %zd, action '%c'\n", i, pc->positions[i].pos, pc->positions[i].action);
    //  }

    if (o_direct == 0) {
        dropCaches();
    }

    keepRunning = 1;

    size_t num = pc->sz;

    if (process) {
        positionContainerCollapse(pc); // this will sort before collapsing using pos sort.
        qsort(pc->positions, pc->sz, sizeof(positionType), timeCompare); // post collapse, sort by time
    }

    pthread_t *pt = NULL;
    CALLOC(pt, threads, sizeof(pthread_t));
    threadInfoType *threadContext;
    CALLOC(threadContext, threads, sizeof(threadInfoType));

    double start = timeAsDouble();

    for (size_t i = 0; i < threads; i++) {
        threadContext[i].job = job;
        threadContext[i].id = i;
        threadContext[i].overridesize = overridesize;
        threadContext[i].numThreads = threads;
        threadContext[i].startInc = (size_t) (i * (num * 1.0 / threads));
        threadContext[i].endExc = (size_t) ((i + 1) * (num * 1.0 / threads));
        threadContext[i].correct = 0;
        threadContext[i].incorrect = 0;
        threadContext[i].ioerrors = 0;
        threadContext[i].lenerrors = 0;
        threadContext[i].pc = pc;
        threadContext[i].bytesRead = 0;
        threadContext[i].iocount = 0;
        threadContext[i].finishTime = finishTime;
        threadContext[i].elapsed = 0;
        threadContext[i].o_direct = o_direct;
        threadContext[i].sorted = 1 || sorted;
        threadContext[i].quiet = quiet;
        threadContext[i].exitearlyafternerrors = exitafternerrorsperthread;

        //        fprintf(stderr,"*info* starting thread[%zd] in range [%zd, %zd)\n", i, threadContext[i].startInc, threadContext[i].endExc);

        if (pthread_create(&(pt[i]), NULL, runThread, &(threadContext[i]))) {
	    fprintTimePrefix(stderr);
	    fprintf(stderr, "*error* can't create a thread\n");
            exit(-1);
        }
    }

    for (size_t i = 0; i < threads; i++) {
        pthread_join(pt[i], NULL);
    }

    double elapsed = timeAsDouble() - start;

    size_t tr = 0;
    size_t correct = 0, incorrect = 0, ioerrors = 0, lenerrors = 0, iocount = 0;

    for (size_t i = 0; i < threads; i++) {
        correct += threadContext[i].correct;
        incorrect += threadContext[i].incorrect;
        ioerrors += threadContext[i].ioerrors;
        lenerrors += threadContext[i].lenerrors;
        iocount += threadContext[i].iocount;

        tr += threadContext[i].bytesRead;
    }
    size_t ops = correct + incorrect + ioerrors + lenerrors;
    assert (ops == iocount);

    if (!quiet)
        fprintf(stderr,
                "*info* verify: correct %zd, incorrect %zd, ioerrors %zd, %zd ops (%.0lf IOPS), %.1lf GB (%.1lf GiB), %.1lf s (threads=%zd), %.1lf MB/s\n",
                correct, incorrect, ioerrors, ops, ops / elapsed, TOGB(tr), TOGiB(tr), elapsed, threads,
                TOMB(tr) / elapsed);

    free(pt);
    free(threadContext);

    if (r_correct) *r_correct = correct;
    if (r_incorrect) *r_incorrect = incorrect;
    if (r_ioerrors) *r_ioerrors = ioerrors;

    return ioerrors + incorrect + lenerrors;
}




