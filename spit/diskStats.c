#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <sys/sysmacros.h>
#include <assert.h>

#include "diskStats.h"
#include "utils.h"

void diskStatSetup(diskStatType *d)
{
  if (!d) return;
  memset(d, 0, sizeof(diskStatType));
}

void diskStatAddDrive(diskStatType *d, int fd, char* suffix)
{
  if (!d) return;
  unsigned int major = 0, minor = 0;
  assert(d);
  majorAndMinor(fd, &major, &minor);
  d->allocDevices++;
  d->majorArray = realloc(d->majorArray, d->allocDevices * sizeof(int));
  d->minorArray = realloc(d->minorArray, d->allocDevices * sizeof(int));
  d->suffixArray = realloc(d->suffixArray, d->allocDevices * sizeof(const char*));
  d->inflightArray = realloc(d->inflightArray, d->allocDevices * sizeof(long));
  //  d->sizeArray = realloc(d->sizeArray, d->allocDevices * sizeof(size_t));

  d->majorArray[d->allocDevices-1] = major;
  d->minorArray[d->allocDevices-1] = minor;
  d->suffixArray[d->allocDevices-1] = suffix;
  d->inflightArray[d->allocDevices-1] = 0;
  //  d->sizeArray[d->allocDevices-1] = blockDeviceSizeFromFD(fd);
  //    fprintf(stderr,"diskStatAddDrive fd %d, major %u, minor %u\n", fd, major, minor);
}



void diskStatAddStart(diskStatType *d, size_t reads, size_t writes)
{
  if (!d) return;
  d->startSecRead += reads;
  d->startSecWrite += writes;
}

void diskStatAddFinish(diskStatType *d, size_t reads, size_t writes)
{
  if (!d) return;
  d->finishSecRead += reads;
  d->finishSecWrite += writes;
}

size_t diskStatTBRead(diskStatType *d)
{
  if (!d) return 0;
  return (d->finishSecRead - d->startSecRead) * 512L;
}

size_t diskStatTBReadIOs(diskStatType *d)
{
  if (!d) return 0;
  return (d->finishIORead - d->startIORead);
}

size_t diskStatTBWrite(diskStatType *d)
{
  if (!d) return 0;
  return (d->finishSecWrite - d->startSecWrite) * 512L;
}

size_t diskStatTBWriteIOs(diskStatType *d)
{
  if (!d) return 0;
  return (d->finishIOWrite - d->startIOWrite);
}


size_t diskStatTBTimeSpentIO(diskStatType *d)   // in ms
{
  if (!d) return 0;
  return d->finishSecTimeio - d->startSecTimeio;
}


void diskStatSummary(diskStatType *d, size_t *totalReadBytes, size_t *totalWriteBytes, size_t *totalReadIO, size_t *totalWriteIO, double *util, size_t shouldReadBytes, size_t shouldWriteBytes, int verbose, double elapsed)
{
  if (!d) return;
  if (d->startSecRead > d->finishSecRead) {
    fprintf(stderr,"start more than fin!\n");
  }
  if (d->startSecWrite > d->finishSecWrite) {
    fprintf(stderr,"start more than fin2!\n");
  }
  *totalReadBytes = (d->finishSecRead - d->startSecRead) * 512L;
  *totalWriteBytes =(d->finishSecWrite - d->startSecWrite) * 512L;
  *totalReadIO = (d->finishIORead - d->startIORead);
  *totalWriteIO = (d->finishIOWrite - d->startIOWrite);

  *util = 100.0 * ((d->finishSecTimeio - d->startSecTimeio)/1000.0 / d->allocDevices) / elapsed;

  if (verbose && (shouldReadBytes || shouldWriteBytes)) {
    if (*totalReadBytes)
      fprintf(stderr,"*info* read  amplification: should be %zd (%.3lf GiB), actual read  %zd (%.3lf GiB), %.2lf%%, %zd device(s)\n", shouldReadBytes, TOGiB(shouldReadBytes), *totalReadBytes, TOGiB(*totalReadBytes), *totalReadBytes*100.0/shouldReadBytes, d->allocDevices);
    if (*totalWriteBytes)
      fprintf(stderr,"*info* write amplification: should be %zd (%.3lf GiB), actual write %zd (%.3lf GiB), %.2lf%%, %zd device(s)\n", shouldWriteBytes, TOGiB(shouldWriteBytes), *totalWriteBytes, TOGiB(*totalWriteBytes), *totalWriteBytes*100.0/shouldWriteBytes, d->allocDevices);
    if (d->allocDevices)
      fprintf(stderr,"*info* total disk utilisation: %.1lf %% (devices = %zd)\n", *util, d->allocDevices);
  }
}


void diskStatUsage(diskStatType *d, size_t *sread, size_t *swritten, size_t *stimeio, size_t *ioread, size_t *iowrite)
{
  if (!d) return;
  diskStatLoadProc(d); // get the latest numbers

  *sread = 0;
  *swritten = 0;
  *stimeio = 0;
  *ioread = 0;
  *iowrite = 0;
  memset(d->inflightArray, -1, d->allocDevices );

  for (size_t j = 0; j < d->deviceCount; j++) { // from /proc/diskstats
    devSnapshotType *s = &d->deviceStats[j];

    for (size_t i = 0; i < d->allocDevices; i++) { // from devices.txt

      if ((d->majorArray[i] == (int)s->major) &&
          (d->minorArray[i] == (int)s->minor)) {

        *sread = (*sread) + s->secRead;
        *swritten = (*swritten) + s->secWrite;
        *stimeio = (*stimeio) + s->secTimeIO;
        *ioread = (*ioread) + s->IORead;
        *iowrite = (*iowrite) + s->IOWrite;
        d->inflightArray[i] = s->inflight;
        // found a match, escape
        break;
      }
    }
  }
}

void diskStatFromFilelist(diskStatType *d, const char *path, int verbose)
{
  if (!d) return;
  FILE *fp = fopen(path, "rt");
  if (!fp) {
    fprintf(stderr,"can't open %s!\n", path);
    exit(1);
  }

  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  char *str;
  CALLOC(str, 1000, 1);
  while ((read = getline(&line, &len, fp)) != -1) {
    if (sscanf(line, "%s", str) >= 1) {
      if (isBlockDevice(str)) {
        int fd = open(str, O_RDONLY);
        if (fd < 0) {
          perror("problem");
        }

        char* real_path = realpath( str, NULL );
        char *suffix = NULL;
        if (real_path) {
          suffix = getSuffix(real_path);
          free(real_path);
        } else {
          suffix = getSuffix(str);
        }

        if (verbose) {
          char *sched = getScheduler(suffix);
          fprintf(stderr,"*info* scheduler for %s is [%s]\n", str, sched);
          free(sched);
        }
        diskStatAddDrive(d, fd, suffix);
        close (fd);
      } else {
        fprintf(stderr,"*warning* %s is not a block device\n", str);
      }
    }
  }
  free(str);
  free(line);
  fclose(fp);
  fprintf(stderr,"*info* specifiedDevices from %s, allocDevices is %zd\n", path, d->allocDevices);
}


void diskStatStart(diskStatType *d)
{
  fprintf(stderr,"*info* diskStats start\n");
  if (!d) return;
  size_t sread = 0, swritten = 0, stimeio = 0, ioread = 0, iowrite = 0;
  diskStatUsage(d, &sread, &swritten, &stimeio, &ioread, &iowrite);
  d->startSecRead = sread;
  d->startSecWrite = swritten;
  d->startSecTimeio = stimeio;
  d->startIORead = ioread;
  d->startIOWrite = iowrite;
}

void diskStatRestart(diskStatType *d)
{
  if (!d) return;
  d->startSecRead = d->finishSecRead;
  d->startSecWrite = d->finishSecWrite;
  d->startSecTimeio = d->finishSecTimeio;
  d->startIORead = d->finishIORead;
  d->startIOWrite = d->finishIOWrite;
}

void diskStatFinish(diskStatType *d)
{
  if (!d) return;
  size_t sread = 0, swritten = 0, stimeio = 0, ioread = 0, iowrite = 0;
  diskStatUsage(d, &sread, &swritten, &stimeio, &ioread, &iowrite);
  d->finishSecRead = sread;
  d->finishSecWrite = swritten;
  d->finishSecTimeio = stimeio;
  d->finishIORead = ioread;
  d->finishIOWrite = iowrite;
}

void diskStatFree(diskStatType *d)
{
  if (!d) return;
  if (d->majorArray) {
    free(d->majorArray);
    d->majorArray = NULL;
  }
  if (d->minorArray) {
    free(d->minorArray);
    d->minorArray = NULL;
  }
  if (d->deviceStats) {
    free(d->deviceStats);
    d->deviceStats = NULL;
  }
  if (d->suffixArray) {
    for (size_t i = 0; i < d->allocDevices; i++ ) {
      if( d->suffixArray[ i ] ) {
        free( d->suffixArray[ i ] );
      }
    }
    free(d->suffixArray);
  }
  if (d->inflightArray) {
    free(d->inflightArray);
    d->inflightArray = NULL;
  }
  d->deviceCount = 0;
  d->allocDevices = 0;
}

void majorAndMinorFromFilename(char *filename, unsigned int *major, unsigned int *minor) {
  int fd = open(filename, O_RDONLY);
  *major = 0;
  *minor = 0;
  if (fd > 0) {
    majorAndMinor(fd, major, minor);
    close(fd);
  }
}

void majorAndMinor(int fd, unsigned int *major, unsigned int *minor)
{
  struct stat buf;
  if (fstat(fd, &buf) == 0) {
    dev_t dt = buf.st_rdev;
    *major = major(dt);
    *minor = minor(dt);
  } else {
    fprintf(stderr,"*warning* can't get major/minor\n");
  }
}

void diskStatLoadProc(diskStatType *d)
{
  FILE *fp = fopen("/proc/diskstats", "rt");
  if (!fp) {
    fprintf(stderr,"can't open diskstats!\n");
    return;
  }
  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  d->deviceCount = 0;
  char str[1000];
  while ((read = getline(&line, &len, fp)) != -1) {
    long mj, mn, s, inflight;
    size_t read1, write1, timespentIO, readcompl1, writecompl1, discardcount1 = 0;
    d->deviceCount++;
    if (d->deviceCount > d->deviceCountAlloc) {
      d->deviceCountAlloc++;
      d->deviceStats = realloc(d->deviceStats, d->deviceCountAlloc * sizeof(devSnapshotType));
    }
    sscanf(line,"%ld %ld %s %zu %ld %zu %ld %zu %ld %zu %ld %ld %zu %zu", &mj, &mn, str, &readcompl1, &s, &read1, &s, &writecompl1, &s, &write1, &s, &inflight, &timespentIO, &discardcount1);
    //    fprintf(stderr,"==%zu\n", discardcount1);
    d->deviceStats[d->deviceCount - 1].major = mj;
    d->deviceStats[d->deviceCount - 1].minor = mn;
    d->deviceStats[d->deviceCount - 1].secRead = read1;
    d->deviceStats[d->deviceCount - 1].secWrite = write1;
    d->deviceStats[d->deviceCount - 1].IORead = readcompl1;
    d->deviceStats[d->deviceCount - 1].IOWrite = writecompl1 + discardcount1;
    d->deviceStats[d->deviceCount - 1].secTimeIO = timespentIO;
    d->deviceStats[d->deviceCount - 1].inflight = inflight;
  }
  free(line);
  fclose(fp);
}

void diskStatInfo(diskStatType *d)
{

  fprintf(stderr,"*info* /proc/diskstat rows %zd\n", d->deviceCount);
  fprintf(stderr,"*info* matching devices %zd\n", d->allocDevices);

  for (size_t i = 0; i < d->allocDevices; i++) {
    fprintf(stderr, "[%zd] major %d, minor %d\n", i, d->majorArray[i], d->minorArray[i]);
  }
}

int compareQDs(const void* a, const void* b)
{
  return *(int*)a < *(int*)b;
}

void diskStatMaxQDStr(diskStatType *d, size_t n_print, char* max_q_disk_str, size_t max_q_disk_str_len)
{
  // Sort inflight
  qsort( d->inflightArray, d->allocDevices, sizeof( d->inflightArray[ 0 ] ), compareQDs );

  // Zero string
  memset( max_q_disk_str, 0, max_q_disk_str_len );

  // Print to string
  size_t count = 0;
  count += sprintf( max_q_disk_str + count, "Max%lu Q ", n_print );

  for( size_t i = 0; i < n_print; i++ ) {
    const char* delim = i == 0 ? "" : "/";
    if( i < d->allocDevices ) {
      count += sprintf( max_q_disk_str + count, "%s%ld", delim, d->inflightArray[ i ] );
    } else {
      count += sprintf( max_q_disk_str + count, "%s-", delim );
    }
  }
}

