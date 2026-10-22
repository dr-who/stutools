#ifndef _PROCDISKSTATS_H
#define _PROCDISKSTATS_H

#include <unistd.h>
#include <stdio.h>

#include "mapVoid.h"

/* What:		/proc/diskstats */
/* Date:		February 2008 */
/* Contact:	Jerome Marchand <jmarchan@redhat.com> */
/* Description: */
/* 		The /proc/diskstats file displays the I/O statistics */
/* 		of block devices. Each line contains the following 14 */
/* 		fields: */

/* 		==  =================================== */
/* 		 1  major number */
/* 		 2  minor mumber */
/* 		 3  device name */
/* 		 4  reads completed successfully */
/* 		 5  reads merged */
/* 		 6  sectors read */
/* 		 7  time spent reading (ms) */
/* 		 8  writes completed */
/* 		 9  writes merged */
/* 		10  sectors written */
/* 		11  time spent writing (ms) */
/* 		12  I/Os currently in progress */
/* 		13  time spent doing I/Os (ms) */
/* 		14  weighted time spent doing I/Os (ms) */
/* 		==  =================================== */

/* 		Kernel 4.18+ appends four more fields for discard */
/* 		tracking putting the total at 18: */

/* 		==  =================================== */
/* 		15  discards completed successfully */
/* 		16  discards merged */
/* 		17  sectors discarded */
/* 		18  time spent discarding */
/* 		==  =================================== */

/* 		Kernel 5.5+ appends two more fields for flush requests: */

/* 		==  ===================================== */
/* 		19  flush requests completed successfully */
/* 		20  time spent flushing */
/* 		==  ===================================== */

/* 		For more details refer to Documentation/admin-guide/iostats. */

typedef struct {
    size_t majorNumber;  // 1
    size_t minorNumber;  // 2
    char *deviceName;    // 3
    size_t readsCompleted; // 4
    size_t readsMerged;  // 5
    size_t sectorsRead;  //6
    size_t timeSpentReading_ms;  //7
    size_t writesCompleted; // 8
    size_t writesMerged;    // 9
    size_t sectorsWritten;  // 10
    size_t timeSpentWriting_ms; // 11
    size_t IOsCurrentInProgress; // 12
    size_t timeSpentDoingIO_ms;  // 13
    size_t weightedTimeSpentDoingIOs_ms; // 14
    size_t discardsCompleteSuccessfully; // 15
    size_t discardsMerged; // 16
    size_t sectorsDiscarded; // 17
    size_t timeSpentDiscarding; // 18

    char *serialShort;
    char *idVendor;
    char *idModel;
} procDiskLineType;

typedef struct {
    size_t num;
    procDiskLineType *devices;
    double sampleTime;
    double startTime;
} procDiskStatsType;

void procDiskStatsInit(procDiskStatsType *d);

void procDiskStatsSample(procDiskStatsType *d);

void procDiskStatsDump(procDiskStatsType *d);

void procDiskStatsDumpThres(FILE *fp, procDiskStatsType *d, float msthres, mapVoidType *map_r, mapVoidType *map_w,
                            const double zscore);

void procDiskStatsFree(procDiskStatsType *d);

procDiskStatsType procDiskStatsDelta(procDiskStatsType *old, procDiskStatsType *new);

void procDiskStatsCopy(procDiskStatsType *new, procDiskStatsType *old);


char *getFieldFromUdev(size_t major, size_t minor, char *match);

#endif

