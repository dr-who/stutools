#define _GNU_SOURCE
#include <stdlib.h>
#include <fcntl.h>
#include <time.h>
#include <sys/time.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include <malloc.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <syslog.h>
#include <errno.h>
#include <limits.h>
#include <unistd.h>
#include <sys/sysinfo.h>
#include <sys/utsname.h>
#include <linux/hdreg.h>
#include "utils.h"

extern int keepRunning;



struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
}

double timedouble() {
  struct timeval now = gettm();
  double tm = (now.tv_sec * 1000000 + now.tv_usec);
  return tm/1000000.0;
}


size_t fileSize(int fd) {
  size_t sz = lseek(fd, 0L, SEEK_END);
  lseek(fd, 0L, SEEK_SET);
  return sz;
}
  
size_t blockDeviceSizeFromFD(int fd) {
  size_t file_size_in_bytes = 0;
  ioctl(fd, BLKGETSIZE64, &file_size_in_bytes);
  return file_size_in_bytes;
}
  

size_t blockDeviceSize(char *path) {

  int fd = open(path, O_RDONLY);
  if (fd < 0) {
    perror(path);
    return 0;
  }
  size_t file_size_in_bytes = 0;
  ioctl(fd, BLKGETSIZE64, &file_size_in_bytes);
  fsync(fd);
  close(fd);
  if (file_size_in_bytes == 0) {
    fprintf(stderr,"*warning*: size is %d bytes?\n", 0);
    file_size_in_bytes = 1; // make it 1 to avoid DBZ
  }
  return file_size_in_bytes;
}



double loadAverage() {
  FILE *fp = fopen("/proc/loadavg", "rt");
  if (fp == NULL) {
    perror("can't open /proc/loadavg");
    return 0;
  }
  double loadavg = 0;
  if (fscanf(fp, "%lf", &loadavg) != 1) {
    fprintf(stderr,"warning: problem with loadavg\n");
  }
  fclose(fp);
  return loadavg;
}


void doChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int writeAction, int sequential, int direct, int verifyWrites, float flushEverySecs, float limitGBToProcess) {

  // check
  //  if (loadAverage() > 10.0) {
  //    fprintf(stderr,"**WARNING** the load average is %g (maybe the machine is busy!?)\n", loadAverage());
  //  }
  
  char *charbuf = aligned_alloc(65536, maxBufSize);
  if (!charbuf) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  srand((int)timedouble());

  generateRandomBuffer(charbuf, maxBufSize);

  size_t checksum = 0;
  for (size_t i = 0; i < maxBufSize; i++ ) {
    checksum += charbuf[i];
  }

  size_t maxDeviceSize = 0;

  if (!sequential) {
    if (isBlockDevice(label)) {
      maxDeviceSize = blockDeviceSize(label);
      if (maxDeviceSize == 0) {
	return;
	//	exit(1);
      }       

      fprintf(stderr,"deviceSize on %s is %.1lf GiB (%.0lf MiB)", label, TOGiB(maxDeviceSize), TOMiB(maxDeviceSize));
      if (limitGBToProcess > 0 && (limitGBToProcess * 1024L * 1024 * 1024 < maxDeviceSize)) {
	maxDeviceSize = limitGBToProcess * 1024L * 1024 * 1024;
	fprintf(stderr,", *override* to %.1lf GiB (%.0lf MiB)", TOGiB(maxDeviceSize), TOMiB(maxDeviceSize));
      }
      fprintf(stderr,"\n");
    } else {
      fprintf(stderr,"error: need to be a block device with the -r option\n");
      exit(1);
    }
  } 
  int wbytes = 0;
  double lastg = timedouble();
  int chunkIndex = 0;
  double startTime = timedouble();
  double lastFdatasync = startTime;
  int resetCount = 1;
  logSpeedType previousSpeeds;

  size_t countValues = 0, allocValues = 20000, sumBytes = 0;
  double *allValues, *allTimes, *allTotal;

  allValues = malloc(allocValues * sizeof(double));
  allTimes = malloc(allocValues * sizeof(double));
  allTotal = malloc(allocValues * sizeof(double));

  logSpeedInit(&previousSpeeds);
  
  while (keepRunning) {
    //    shmemWrite(); // so other people know we're running!

    if (!sequential) {
      size_t maxblocks = maxDeviceSize / chunkSizes[0];
      const size_t randblock = maxblocks ? rand() % maxblocks : 0;
      //      fprintf(stderr,"%zd (0..%zd)\n", randblock, maxblocks);	
      size_t pos = randblock * chunkSizes[0];
      off_t ret = lseek(fd, pos, SEEK_SET);
      if (ret < 0) {
	perror("seek error");
      }
    }

    if (writeAction) {
      wbytes = write(fd, charbuf, chunkSizes[0]);
      if (wbytes < 0) {
	//	perror("problem writing");
	lseek(fd, 0, SEEK_SET);
	continue;
	break;
      }
    } else {
      wbytes = read(fd, charbuf, chunkSizes[0]);
      if (wbytes < 0) {
	//	perror("problem reading");
	lseek(fd, 0, SEEK_SET);
	continue;
	break;
      }
    }
    if (wbytes == 0) {
      //      fprintf(stderr,"eod: rewinding\n");
      lseek(fd, 0, SEEK_SET); // run until timeout
      continue;
      //     break;
    }

    const double tt = timedouble();
    
    logSpeedAdd(l, wbytes);

    sumBytes += wbytes;
    allValues[countValues] = wbytes;
    allTimes[countValues] = tt;
    allTotal[countValues] = sumBytes;
    countValues++;
    
    if (countValues >= allocValues) {
      allocValues = allocValues * 2 + 2;
      double *ret;
      ret = realloc(allValues, allocValues * sizeof(double)); if (ret) allValues = ret; // too big, so just stop growing and continue
      ret = realloc(allTimes, allocValues * sizeof(double)); if (ret) allTimes = ret;
      ret = realloc(allTotal, allocValues * sizeof(double)); if (ret) allTotal = ret;
    }

    if ((flushEverySecs > 0) && (tt - lastFdatasync > flushEverySecs)) {
      fprintf(stderr,"fdatasync() at %.1lf seconds\n", tt - startTime);
      fdatasync(fd);
      lastFdatasync = tt;
    }
    
    if ((tt - lastg) >= outputEvery) {
      logSpeedAdd(&previousSpeeds, logSpeedMean(l)); // after a second, add the mean of the time period to an array of speeds
      double deltabytes = logSpeedTotal(l) - logSpeedGetCheckpoint(l);

      fprintf(stderr,"%s '%s': %.1lf GiB, mean %.1f MiB/s, median %.1f MiB/s, 1%% %.1f MiB/s, 95%% %.1f MiB/s, n=%zd, %.1fs\n", writeAction ? "write" : "read", label, TOGiB(l->total), TOMiB(deltabytes / (tt - lastg)), TOMiB(logSpeedMedian(l)), TOMiB(logSpeedRank(l, 0.01)), TOMiB(logSpeedRank(l, 0.95)), l->num, tt - l->starttime);

      lastg = tt;
      logSpeedCheckpoint(&previousSpeeds); // tag the current total so mean
      logSpeedCheckpoint(l); // tag the current total so mean
      if ((logSpeedN(&previousSpeeds) >= resetTime) && (resetCount > 0)) { // at least 10 data points before a reset
	if (keepRunning) {
	  /*double low = logSpeedRank(&previousSpeeds, .1);
	  double high = logSpeedRank(&previousSpeeds, .9);
	  double mean = logSpeedMean(&previousSpeeds);
	  	  if ((high / low > 1.05) && (mean < low || mean > high) && (resetCount > 0)) { // must be over 5% difference
	    fprintf(stderr,"  [ %.1lf < %.1lf MiB/s < %.1lf ]\n", TOMiB(low), TOMiB(mean), TOMiB(high));
	    fprintf(stderr,"resetting due to volatile timings (%d resets remain)\n", resetCount); */
	    resetCount--;
	    startTime = tt;
	    logSpeedReset(l);
	    logSpeedReset(&previousSpeeds);
	    //	    }
	}
      }
    }
    if (chunkIndex >= numChunks) {
      chunkIndex = 0;
    }
    if ((resetCount == 0) && maxTime && (tt - startTime > maxTime)) {
      //fprintf(stderr,"timer triggered after %zd seconds\n", maxTime);
      break;
    }
  } // while loop
  double startclose = timedouble();
  if ( (timedouble() - startclose > 0.1)) {
    fprintf(stderr,"flushing and closing..."); fflush(stderr);
    fdatasync(fd);
    fsync(fd);
    fprintf(stderr,"took %.1f seconds\n", timedouble() - startclose);
  } 
  close(fd);
    

  // add the very last value
  if (countValues < allocValues) {
    allValues[countValues] = wbytes;
    allTimes[countValues] = timedouble();
    allTotal[countValues] = sumBytes;
    countValues++;
  }

  l->lasttime = timedouble(); // change time after closing
  //  if (resetCount > 0) {
    char s[1000], *osr = OSRelease();
    sprintf(s, "Total %s '%s': %.1lf GiB, %.1f s, mean %.1f MiB/s, %d B (%d KiB), %s, %s, n=%zd%s\n", writeAction ? "write" : "read", label, TOGiB(l->total), logSpeedTime(l), TOMiB(logSpeedMean(l)), chunkSizes[0], chunkSizes[0] / 1024, sequential ? "seq" : "rand", direct ? "DIRECT" : "NOT DIRECT (pagecache)", countValues, keepRunning ? "" : " ^C");
    fprintf(stdout, "%s", s);
    char *user = username();
    syslog(LOG_INFO, "%s - %s", user, s);
    free(user);
    free(osr);
    /*  } else {
    fprintf(stderr,"error: results too volatile. Perhaps the machine is busy?\n");
    }*/

  // dump all values to a log file
    //  char s[1000];
  sprintf(s, "log-%dKB-%s-%s-%s--%.1lf-MiB_s-%s", chunkSizes[0]/1024, direct ? "direct" : "not-direct", sequential ? "seq" : "rand", label, TOMiB(logSpeedMean(l)), writeAction ? "write" : "read");
  for (size_t i = 0; i < strlen(s); i++) {
    if (s[i] == '/') {
      s[i] = '-';
    }
  }
  double firsttime = 0;
  if (countValues > 0) {
    firsttime = allTimes[0];
  }

  FILE *fp = fopen(s, "wt"); 
  if (fp) {
    //    fprintf(stderr,"writing log/stats file: '%s'\n", s);
    fprintf(fp,"#time    \tbigtime             \tchunk\ttotalbytes\n");
    for (size_t i = 0; i < countValues; i++) {
      int ret = fprintf(fp,"%.6lf\t%.6lf\t%.0lf\t%.0lf\n", allTimes[i] - firsttime, allTimes[i], allValues[i], allTotal[i]);
      if (ret <= 0) {
	fprintf(stderr,"error: trouble writing log file\n");
	break;
      }
    }
    if (fflush(fp) != 0) {perror("problem flushing");}
    if (fclose(fp) != 0) {perror("problem closing");}
  } else {
    perror("problem creating logfile");
  }
	  
  if (allValues) free(allValues);
  if (allTimes) free(allTimes);
  if (allTotal) free(allTotal);

  // now verify
  if (verifyWrites) {
    if (sequential) {
      checkContents(label, charbuf, chunkSizes[0], checksum, 1, l->total);
    } else {
      fprintf(stderr,"verify random writes not implemented. use aioRWTest\n");
    }
  }
  
  if (charbuf) free(charbuf);
  
  //  logSpeedHistogram(&previousSpeeds);
  logSpeedFree(&previousSpeeds);
}


void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess, int verifyWrites, float flushEverySecs) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, resetTime, l, maxBufSize, outputEvery, 1, seq, direct, verifyWrites, flushEverySecs, limitGBToProcess);
}

void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess) {
  doChunks(fd, label, chunkSizes, numChunks, maxTime, resetTime, l, maxBufSize, outputEvery, 0, seq, direct, 0, 0, limitGBToProcess);
}


int isBlockDevice(char *name) {
  struct stat sb;

  if (stat(name, &sb) == -1) {
    return 0;
  }
  return (S_ISBLK(sb.st_mode));
}


void dropCaches() {
  FILE *fp = fopen("/proc/sys/vm/drop_caches", "wt");
  if (fp == NULL) {
    fprintf(stderr,"error: you need sudo/root permission to drop caches\n");
    exit(1);
  }
  if (fprintf(fp, "3\n") < 1) {
    fprintf(stderr,"error: you need sudo/root permission to drop caches\n");
    exit(1);
  }
  fflush(fp);
  fclose(fp);
  fprintf(stderr,"*info* /proc/sys/vm/drop_caches dropped\n");
}


char* queueType(char *path) {
  if (path) {
  }
  FILE *fp = fopen("/sys/block/sda/device/queue_type", "rt");
  if (fp == NULL) {
    perror("problem opening");
    //    exit(1);
  }
  char instr[100];
  size_t r = fscanf(fp, "%s", instr);
  fclose(fp);
  if (r != 1) {
    return strdup("n/a");
    //    fprintf(stderr,"error: problem reading from '%s'\n", path);
    //    exit(1);
  }
  return strdup(instr);
}


char *username() {
  char *buf = NULL;
  CALLOC(buf, 200, sizeof(char));
  getlogin_r(buf, 200);
  return buf;
}


void checkContents(char *label, char *charbuf, size_t size, const size_t checksum, float checkpercentage, size_t stopatbytes) {
  fprintf(stderr,"verifying contents of '%s'...\n", label);
  if (charbuf || checkpercentage) {
  }
  int fd = open(label, O_RDONLY | O_DIRECT); // O_DIRECT to check contents
  if (fd < 0) {
    perror("checkContents");
    exit(1);
  }

  void *rawbuf = NULL;
  if ((rawbuf = aligned_alloc(65536, size)) == NULL) { // O_DIRECT requires aligned memory
	fprintf(stderr,"memory allocation failed\n");exit(1);
  }
  size_t pos = 0;
  unsigned char *buf = (unsigned char*)rawbuf;
  unsigned long ii = (unsigned long)rawbuf;
  size_t check = 0, ok = 0, error = 0;
  srand(ii);

  keepRunning = 1;
  while (keepRunning) {
    if (pos >= stopatbytes) {
      break;
    }
    int wbytes = read(fd, buf, size);
    if (wbytes == 0) {
      break;
    }
    if (wbytes < 0) {
      perror("problem reading");
      break;
    }
    if ((size_t)wbytes == size) { // only check the right size blocks
      check++;
      size_t newsum = 0;
      for (size_t i = 0; i < (size_t)wbytes;i++) {
	newsum += (buf[i] & 0xff);
      }
      if (newsum != checksum) {
	error++;
	//	fprintf(stderr,"X");
	if (error < 5) {
	  fprintf(stderr,"checksum error %zd\n", pos);
	  //	  fprintf(stderr,"buffer: %s\n", buf);
	}
	if (error == 5) {
	  fprintf(stderr,"further errors not displayed\n");
	}
      } else {
	//	fprintf(stderr,".");
	ok++;
      }
      //    } else {
      //      fprintf(stderr,"eek bad aligned read\n");
    }
    pos += wbytes;
  }
  fflush(stderr);
  close(fd);

  char *user = username();
  syslog(LOG_INFO, "%s - verify '%s': %.1lf GiB, checked %zd, correct %zd, failed %zd\n", user, label, TOGiB(size*check), check, ok, error);

  fprintf(stderr, "verify '%s': %.1lf GiB, checked %zd, correct %zd, failed %zd\n", label, TOGiB(size*check), check, ok, error);

  if (error > 0) {
    syslog(LOG_INFO, "%s - checksum errors on '%s'\n", user, label);
    fprintf(stderr, "**CHECKSUM** errors\n");
  }
  free(user);
  free(rawbuf);
}


size_t numThreads() {
  return sysconf(_SC_NPROCESSORS_ONLN);
}

size_t totalRAM() {
  struct sysinfo info;
  sysinfo(&info);
  return info.totalram;
}

char *OSRelease() {
  struct utsname buf;
  uname (&buf);
  return strdup(buf.release);
}

int getWriteCacheStatus(int fd) {
  unsigned long val = 0;
  if (ioctl(fd, HDIO_GET_WCACHE, &val) >= 0) {
    fprintf(stderr,"*info* write cache setting for %d is %lu\n", fd, val);
  } else {
    perror("ioctl");
  }
  return val;
}

int trimDevice(int fd, char *path, unsigned long low, unsigned long high) {
  unsigned long range[2];
  
  range[0] = low;
  range[1] = high;

  //  fprintf(stderr,"*info* sending trim command to %s [%ld, %ld] [%.1lf GiB, %.1lf GiB]\n", path, range[0], range[1], TOGiB(range[0]), TOGiB(range[1]));
  
  int err = 0;
  if ((err = ioctl(fd, BLKDISCARD, &range))) {
    fprintf(stderr, "%s: BLKDISCARD ioctl failed\n", path);
  }

  fdatasync(fd);
  fsync(fd);

  return err;
}

// the block size random buffer. Nice ASCII
void generateRandomBuffer(char *buffer, size_t size) {

  time_t timer;
  char timebuffer[26];
  struct tm* tm_info;
  
  time(&timer);
  tm_info = localtime(&timer);
  
  strftime(timebuffer, 26, "%Y-%m-%d %H:%M:%S", tm_info);
  
      
  char *user = username();

  const char verystartpoint = ' ' + (lrand48() % 15);
  const char jump = (lrand48() % 3) + 1;
  char startpoint = verystartpoint;
  for (size_t j = 0; j < size; j++) {
    buffer[j] = startpoint;
    startpoint += jump;
    if (startpoint > 'z') {
      startpoint = verystartpoint;
    }
  }
  buffer[size - 1] = '\n'; // end of string to help printing
  char s[1000];
  sprintf(s, "stutools - %s - %s\n", user, timebuffer);
  strncpy(buffer, s, strlen(s));

  free(user);
  buffer[size - 1] = 0;

  if (strlen(buffer) != size -1) { // size will be one off because of the last value being a 0
    fprintf(stderr,"eekk random!\n");
  }
  //  if (verbose >= 2) {
  //    fprintf(stderr,"generated randomBuffer: %s\n", buffer);
  //  }
}


/* creates a new string */
char *getSuffix(const char *path) {
  int found = -1;
  for (size_t i = strlen(path)-1; i >= 0; i--) {
    if (path[i] == '/') {
      found = i + 1;
      break;
    }
  }
  if (found > 0) {
    return strdup(path + found);
  } else {
    return NULL;
  }
}


char *getScheduler(const char *path) {
  char *suffix = getSuffix(path); 
  if (suffix) {
    char s[1000];
    sprintf(s, "/sys/block/%s/queue/scheduler", suffix);
    free(suffix);
    FILE *fp = fopen(s, "rt"); 
    if (!fp) {
      //      perror("scheduler");
      return strdup("problem");
    }
    //    fprintf(stderr,"opened %s\n", s);
    int ret = fscanf(fp, "%s", s);
    fclose(fp);
    if (ret == 1) {
      return strdup(s);
    }
  }

  return strdup("unknown");
}
