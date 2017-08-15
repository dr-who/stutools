#include <stdio.h>
#include "diskStats.h"
#include "utils.h"


void diskStatSetup(diskStatType *d) {
  d->startSecRead = 0;
  d->startSecWrite = 0;
  d->finisSecRead = 0;
  d->finisSecWrite = 0;
}

void diskStatAddStart(diskStatType *d, size_t readSectors, size_t writeSectors) {
  d->startSecRead += readSectors;
  d->startSecWrite += writeSectors;
}

void diskStatAddFinish(diskStatType *d, size_t readSectors, size_t writeSectors) {
  d->finisSecRead += readSectors;
  d->finisSecWrite += writeSectors;
}


void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose) {
  *totalReadBytes = (d->finisSecRead - d->startSecRead) * 512;
  *totalWriteBytes =(d->finisSecWrite - d->startSecWrite) * 512;
  if (verbose && (shouldReadBytes || shouldWriteBytes)) {
    fprintf(stderr,"*info* read  amplification: should be %zd, read  %zd, %.2lf%%\n", shouldReadBytes, *totalReadBytes, *totalReadBytes*100.0/shouldReadBytes);
    fprintf(stderr,"*info* write amplification: should be %zd, write %zd, %.2lf%%\n", shouldWriteBytes, *totalWriteBytes, *totalWriteBytes*100.0/shouldWriteBytes);
  }
}



