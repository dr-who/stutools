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
#include <pthread.h>
#include <unistd.h>
#include <sys/sysinfo.h>
#include <sys/utsname.h>
#include <sys/types.h>
#include <pwd.h>
#include <linux/hdreg.h>
#include <assert.h>

#include "utils.h"

extern int keepRunning;



/*struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
  }*/

inline double timedouble() {
  struct timeval now;
  gettimeofday(&now, NULL);
  double tm = ((double)now.tv_sec * 1000000.0) + now.tv_usec;
  assert(tm > 0);
  return tm/1000000.0;
}

size_t fileSize(int fd) {
  size_t sz = lseek(fd, 0L, SEEK_END);
  lseek(fd, 0L, SEEK_SET);
  return sz;
}

size_t fileSizeFromName(const char *path) {
  int fd = open(path, O_RDONLY);
  if (fd < 0) {
    //    fprintf(stderr,"*info* no file present '%s'\n", path);
    //    perror(path);
    return 0;
  }
  size_t sz = lseek(fd, 0L, SEEK_END);
  lseek(fd, 0L, SEEK_SET);
  close(fd);
  return sz;
}

size_t fileExists(const char *path) {
  int fd = open(path, O_RDONLY);
  if (fd >= 0) {
    close(fd);
    return 1;
  } else {
    return 0;
  }
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


int isBlockDevice(const char *name) {
  struct stat sb;
  int ret;

  if ((ret = stat(name, &sb)) == -1) {
    //    fprintf(stderr,"*warning* isBlockDevice '%s' returned %d\n", name, ret);
    return 0;
  }

  switch (sb.st_mode & S_IFMT) { // check the filetype bits
  case S_IFBLK:  ret=1; break; //printf("block device\n");            break;
  case S_IFCHR:  ret=0; break; // printf("character device\n");        break;
  case S_IFDIR:  ret=0; break; // printf("directory\n");               break;
  case S_IFIFO:  ret=0; break; // printf("FIFO/pipe\n");               break;
  case S_IFLNK:  ret=0; break; // retprintf("symlink\n");                 break;
  case S_IFREG:  ret=2; break; // printf("regular file\n");            break;
  case S_IFSOCK: ret=0; break ; //printf("socket\n");                  break;
  default:   ret=0; break; //    printf("unknown?\n");                break;
  }
    
  return ret;
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


size_t numThreads() {
  return sysconf(_SC_NPROCESSORS_ONLN);
}

size_t totalRAM() {
  struct sysinfo info;
  sysinfo(&info);
  return info.totalram;
}

size_t getUptime() {
  struct sysinfo info;
  sysinfo(&info);
  return info.uptime;
}

size_t freeRAM() {
  struct sysinfo info;
  sysinfo(&info);
  return info.freeram;
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

int getWriteCache(const char *suf) {
  char s[200],s2[200];
  int ret = 0;
  FILE *fp = NULL;
  if (suf) {
    sprintf(s, "/sys/block/%s/queue/write_cache",  suf);
    fp = fopen(s, "rt");
    if (!fp) {
      ret = -1;
      goto wvret;
    }
    
    if (fscanf(fp, "%s %s", s, s2) == 2) {
      if (strcasecmp(s2, "back") == 0) {
	// write back, only for consumer testing
	ret = 1;
	goto wvret;
      }
    }
  } else {
    // can't get a suffix
    ret = -2;
  }
  // write through is ret code 0
 wvret:
  if (fp) fclose(fp);
  return ret;
}
  
  

char *hostname() {
  char s[1000];
  gethostname(s, 999);
  if (strchr(s,'.')) {
    *strchr(s,'.') = 0;
  }
  return strdup(s);
}


// the block size random buffer. Nice ASCII
void generateRandomBufferCyclic(char *buffer, size_t size, unsigned short seedin, size_t cyclic) {

  if (cyclic > size || cyclic == 0) cyclic = size;

  if (cyclic != size) {
    //fprintf(stderr,"*info* generating a random buffer with a size %zd bytes, cyclic %zd bytes\n", size, cyclic);
  }

  unsigned int seed = seedin;
  
  const char verystartpoint = ' ' + (rand_r(&seed) % 30);
  const char jump = (rand_r(&seed) % 3) + 1;
  char startpoint = verystartpoint;
  for (size_t j = 0; j < cyclic; j++) {
    buffer[j] = startpoint;
    startpoint += jump;
    if (startpoint > 'z') {
      startpoint = verystartpoint;
    }
  }

  char s[1000];
  memset(s, 0, 1000);
  const size_t topr = sprintf(s, "________________stutools - %s - %u\n", "spit", seed);
  strncpy(buffer, s, topr);
  buffer[cyclic-2] = '\n';
  buffer[cyclic-1] = 0;

  for (size_t j = cyclic; j < size; j++) {
    buffer[j] = buffer[j % cyclic];
  }
}


void generateRandomBuffer(char *buffer, size_t size, unsigned short seed) {
  if (seed == 0) printf("ooon\n");
  generateRandomBufferCyclic(buffer, size, seed, size);
}


/* creates a new string */
char *getSuffix(const char *path) {
  if (!path) return NULL;
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



char *getModel(const char *suffix) {
  if (suffix) {
    size_t len = 1000;
    char *s;
    CALLOC(s, len, 1);

    sprintf(s, "/sys/block/%s/device/model", suffix);
    FILE *fp = fopen(s, "rt"); 
    if (fp) {
      int ret = getline(&s, &len, fp);
      fclose(fp);
      
      if (ret > 1) {
	s[ret - 1] = 0;
	return s;
      }
    } else {
      free(s);
      return strdup("MISSING MODEL STRING in /sys/block/../device/model");
    }
  }

  return NULL;
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
      if (ret == 1) {
	*phy = d;
      }
      if (fp) {
	fclose(fp); fp = NULL;
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
      if (ret == 1) {
	*log = d;
      }
      if (fp) {
	fclose(fp); fp = NULL;
      }
    }
  }
}


size_t alignedNumber(size_t num, size_t alignment) {
  size_t ret = num;
  if (alignment) {
    
    ret = num / alignment;
    //    fprintf(stderr,"num %zd / %zd = ret %zd\n", num, alignment, ret);
    if (num % alignment > num/2) {
      ret++;
    }
    ret = ret * alignment;

    //      fprintf(stderr,"requested %zd returned %zd\n", num, ret);
  }

  return ret;
}

// return the blockSize
inline size_t randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits, size_t randomValue) {
  if (highbsBytes == 0) {
    return 0;
  }
  
  assert(alignmentbits < 100);

  size_t lowbs_k = lowbsBytes >> alignmentbits; // 1 / 4096 = 0
  //  if (lowbs_k < 1) lowbs_k = 1;
  size_t highbs_k = highbsBytes >> alignmentbits;   // 8 / 4096 = 2
  //  if (highbs_k < 1) highbs_k = 1;
  
  size_t randombs_k = lowbs_k;
  if (highbs_k > lowbs_k) {
    randombs_k += (randomValue % (highbs_k - lowbs_k + 1));
  }

  size_t randombs = randombs_k << alignmentbits;

  assert(randombs >= lowbsBytes);
  assert(randombs <= highbsBytes);
  //  fprintf(stderr,"random bytes %zd\n", randombs);
  return randombs;
}

int startsWith(const char *pre, const char *str)
{
  size_t lenpre = strlen(pre), lenstr = strlen(str);
  return lenstr < lenpre ? 0 : strncmp(pre, str, lenpre) == 0;
}


int canOpenExclusively(const char *fn) {
  int fd = open(fn, O_RDWR | O_EXCL);
  if (fd < 0) {
    return 0;
  }
  close(fd);
  return 1;
}


size_t canCreateFile(const char *filename, const size_t sz) {
  fprintf(stderr,"*info* creating '%s', size %zd bytes (%.3g GiB)\n", filename, sz, TOGiB(sz));
  int fd = open(filename, O_RDWR | O_CREAT | O_DIRECT | O_TRUNC, S_IRUSR | S_IWUSR);
  if (fd < 0)
    return 0;
#ifdef FALLOC_FL_ZERO_RANGE
  fallocate(fd, FALLOC_FL_ZERO_RANGE, 0, sz);
  size_t create_size = fileSizeFromName(filename);
#else
  size_t create_size = sz;
#endif
  
  close(fd);
  remove(filename);
  return create_size;
}


int createFile(const char *filename, const size_t sz) {
  if (!filename) {
    fprintf(stderr,"*error* no filename\n");
    exit(1);
  }
  assert(sz);

  if (startsWith("/dev/", filename)) {
    fprintf(stderr,"*error* path error, will not create a file '%s' in the directory /dev/\n", filename);
    exit(-1);
  }

  /*  size_t create_sz = canCreateFile(filename, sz);
  if (create_sz) { // if it fallocated any file at all 
    if (create_sz != sz) { // and if it's wrong size
      fprintf(stderr,"*error* can't create filename '%s', limited to size %zd (%.1lf GiB)\n", filename, create_sz, TOGiB(create_sz));
      exit(-1);
    }
    }*/

  double timestart = timedouble();
  int fd = open(filename, O_RDWR | O_CREAT | O_TRUNC | O_DIRECT, S_IRUSR | S_IWUSR);
  if (fd < 0) {
    fprintf(stderr,"*warning* creating file with O_DIRECT failed (no filesystem support?)\n");
    fprintf(stderr,"*warning* parts of the file will be in the page cache/RAM.\n");
    fd = open(filename, O_RDWR | O_CREAT | O_TRUNC, S_IRUSR | S_IWUSR);
    if (fd < 0) {
      perror(filename);return 1;
    }
  }

  keepRunning = 1;
  char *buf = NULL;

#define CREATECHUNK (1024*1024)
  
  CALLOC(buf, 1, CREATECHUNK);
  generateRandomBuffer(buf, CREATECHUNK, 42);
  fprintf(stderr,"*info* slow writing '%s' %zd (%.3lf GiB)\n", filename, sz, TOGiB(sz));
  size_t towriteMiB = sz, totalw = 0;

  while (towriteMiB > 0 && keepRunning) {
    int towrite = MIN(towriteMiB, CREATECHUNK);
    int wrote = write(fd, buf, towrite);
    totalw += wrote;
    //    fprintf(stderr,"wrote %zd, %.3lf MB/s\n", totalw, TOMB(wrote / (timedouble() - timestart)));
    if (wrote < 0) {
      perror("createFile");free(buf);return 1;
    }
    towriteMiB -= wrote;
  }
  //  fprintf(stderr,"*info* wrote down to %zd\n", towriteMiB);
  fsync(fd);
  close(fd);
  free(buf);
  double timeelapsed = timedouble() - timestart;
  fprintf(stderr,"*info* file '%s' created in %.1lf seconds, %.0lf MB/s\n", filename, timeelapsed, TOMB(sz / timeelapsed));
  if (!keepRunning) {
    fprintf(stderr,"*warning* early size creation termination\n");
    exit(-1);
  }
  return 0;
}

void commaPrint0dp(FILE *fp, double d) {
  if (d >= 1000) {
    size_t dd = d;
    if (d >= 1000000000) {
      fprintf(fp,"%zd,%03zd,%03zd,%03zd", (dd / 1000000000), (dd % 1000000000)/1000000, (dd % 1000000)/1000, dd % 1000);
    } else if (d >= 1000000) {
      fprintf(fp,"%.0lf,%03zd,%03zd", d/1000000, (dd % 1000000)/1000, dd % 1000);
    } else {
      fprintf(fp,"%3zd,%03zd", dd / 1000, dd % 1000);
    }
  } else {
    fprintf(fp,"%7.0lf", d);
  }
}


// 0 means no values, 1 means one, 2 means two values

int splitRange(const char *charBS, double *low, double *high) {
  int retvalue = 0;
  if (charBS) {
    
    char *endp = NULL;
    *low = strtod(charBS, &endp);
    *high = *low;
    retvalue = 1;
    if (*endp == '-') {
      *high = atof(endp+1);
      retvalue = 2;
    }
  }
  return retvalue;
}
