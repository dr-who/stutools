#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <stdio.h>
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
#include <assert.h>

#include "utils.h"

extern int keepRunning;



struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
}

double timedouble() {
  struct timeval now = gettm();
  double tm = ((double)now.tv_sec * 1000000.0 + now.tv_usec);
  assert(tm > 0);
  return tm/1000000.0;
}


size_t fileSize(int fd) {
  size_t sz = lseek(fd, 0L, SEEK_END);
  lseek(fd, 0L, SEEK_SET);
  return sz;
}
  
size_t blockDeviceSizeFromFD(const int fd) {
  size_t file_size_in_bytes = 0;
  ioctl(fd, BLKGETSIZE64, &file_size_in_bytes);
  return file_size_in_bytes;
}
  

size_t blockDeviceSize(const char *path) {

  int fd = open(path, O_RDONLY);
  if (fd < 0) {
    //    perror(path);
    //    exit(-1);
    return 0;
  }
  size_t file_size_in_bytes = 0;
  ioctl(fd, BLKGETSIZE64, &file_size_in_bytes);
  fsync(fd);
  close(fd);
  if (file_size_in_bytes == 0) {
    fprintf(stderr,"*warning*: block device '%s' has a size of %d bytes.\n", path, 0);
    file_size_in_bytes = 1; // make it 1 to avoid DBZ
  }
  return file_size_in_bytes;
}


size_t swapTotal() {

  FILE *fp = fopen("/proc/swaps", "rt");
  if (fp == NULL) {perror("/proc/swaps");return 0;}
  
  size_t ts= 0;

  char *line = NULL;
  size_t len = 0;
  ssize_t read = 0;
  
  while ((read = getline(&line, &len, fp)) != -1) {
    if (line[0] == '/') {
      // a /dev line
      size_t size;
      char name[1000], part[1000];
      int s = sscanf(line, "%s %s %zu", name, part, &size);
      if (s == 3) {
	// in /proc the size is in KiB
	ts += (size << 10);
      }
    }
  }
  
  free(line);
  fclose(fp);

  return ts;
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


void writeChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess, int verifyWrites, float flushEverySecs) {
  //  doChunks(fd, label, chunkSizes, numChunks, maxTime, resetTime, l, maxBufSize, outputEvery, 1, seq, direct, verifyWrites, flushEverySecs, limitGBToProcess);
}

void readChunks(int fd, char *label, int *chunkSizes, int numChunks, size_t maxTime, size_t resetTime, logSpeedType *l, size_t maxBufSize, size_t outputEvery, int seq, int direct, float limitGBToProcess) {
  //  doChunks(fd, label, chunkSizes, numChunks, maxTime, resetTime, l, maxBufSize, outputEvery, 0, seq, direct, 0, 0, limitGBToProcess);
}


int isBlockDevice(const char *name) {
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
    exit(-1);
  }
  if (fprintf(fp, "3\n") < 1) {
    fprintf(stderr,"error: you need sudo/root permission to drop caches\n");
    exit(-1);
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
    //    exit(-1);
  }
  char instr[100];
  size_t r = fscanf(fp, "%s", instr);
  fclose(fp);
  if (r != 1) {
    return strdup("n/a");
    //    fprintf(stderr,"error: problem reading from '%s'\n", path);
    //    exit(-1);
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
    perror(label);
    exit(-1);
  }

  char *rawbuf = NULL;
  CALLOC(rawbuf, 1, size);

  size_t pos = 0;
  unsigned char *buf = (unsigned char*)rawbuf;
  //  unsigned long ii = (unsigned long)rawbuf;
  size_t check = 0, ok = 0, error = 0;
  //  srand(ii);

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


// the block size random buffer. Nice ASCII
void generateRandomBuffer(char *buffer, size_t size, long seed) {

  //  time_t timer;
  //  char timebuffer[26];
  //  struct tm* tm_info;
  
  //  time(&timer);
  //  tm_info = localtime(&timer);
  
  //  strftime(timebuffer, 26, "%Y-%m-%d %H:%M:%S", tm_info);

  //  fprintf(stderr,"*info* start %s localtime\n", timebuffer);

  //fprintf(stderr,"*info* generating random size %zd, seed %ld\n", size, seed);
  
  srand48(seed);
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
  buffer[size - 1] = 0; // end of string to help printing
  char s[1000];
  memset(s, 0, 1000);
  const size_t topr = sprintf(s, "stutools - %s - %ld\n", user == NULL ? "" : user, seed);
  strncpy(buffer, s, topr);

  free(user);
  if (size > 0)
    buffer[size - 1] = 0;

  //  fprintf(stderr,"size: %zd, jump %d, '%s'\n", size, jump, buffer);
}


/* creates a new string */
char *getSuffix(const char *path) {
  int found = -1;
  for (size_t i = strlen(path)-1; i > 0; i--) {
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


char *getScheduler(const char *suffix) {
  if (suffix) {
    char s[1000];
    sprintf(s, "/sys/block/%s/queue/scheduler", suffix);
    FILE *fp = fopen(s, "rt"); 
    if (!fp) {
      //      perror(s);
      return strdup("problem");
    }
    //    fprintf(stderr,"opened %s\n", s);
    int ret = fscanf(fp, "%s", s);
    fclose(fp);
    if (ret == 1) {
      return strdup(s);
    }
  }

  return strdup("NULL");
}


void getPhyLogSizes(const char *suffix, size_t *phy, size_t *log) {
  *phy = 512;
  *log = 512;
  if (suffix) {
    char s[1000];
    // first physical
    sprintf(s, "/sys/block/%s/queue/physical_block_size", suffix);
    int d, ret;
    FILE *fp = fopen(s, "rt"); 
    if (!fp) {
      //      fprintf(stderr,"*error* problem opening %s: returning 512\n", s);
    } else {
    //    fprintf(stderr,"opened %s\n", s);
      ret = fscanf(fp, "%d", &d);
      fclose(fp);
      if (ret == 1) {
	*phy = d;
      }
    }

      // first physical
    sprintf(s, "/sys/block/%s/queue/logical_block_size", suffix);
    fp = fopen(s, "rt"); 
    if (!fp) {
      //      fprintf(stderr,"*error* problem opening %s: returning 512\n", s);
      *log = 512;
    } else {
      //    fprintf(stderr,"opened %s\n", s);
      ret = fscanf(fp, "%d", &d);
      fclose(fp);
      if (ret == 1) {
	*log = d;
      }
    }
  }
}


size_t alignedNumber(size_t num, size_t alignment) {
  size_t ret = num / alignment;
  if (num % alignment > num/2) {
    ret++;
  }
  ret = ret * alignment;

  //  fprintf(stderr,"requested %zd returned %zd\n", num, ret);

  return ret;
}

// return the blockSize
inline size_t randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits) {
  assert(alignmentbits < 100);

  size_t lowbs_k = lowbsBytes >> alignmentbits; // 1 / 4096 = 0
  if (lowbs_k < 1) lowbs_k = 1;
  size_t highbs_k = highbsBytes >> alignmentbits;   // 8 / 4096 = 2
  if (highbs_k < 1) highbs_k = 1;
  
  size_t randombs_k = lowbs_k;
  if (highbs_k > lowbs_k) {
    randombs_k += (lrand48() % (highbs_k - lowbs_k + 1));
  }
  
  size_t randombs = randombs_k << alignmentbits;
  if (randombs <= 0) {
    randombs = 1 << alignmentbits;
  }
  //  fprintf(stderr,"random bytes %zd\n", randombs);
  return randombs;
}
