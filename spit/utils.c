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


char *username() {

  return strdup(getpwuid(geteuid())->pw_name);
    
  //  char *buf = NULL;
  //  CALLOC(buf, 200, sizeof(char));
  //  getlogin_r(buf, 200);
  //  buf = cuserid(buf);
  //  return buf;
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
  size_t check = 0, ok = 0, error = 0;

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
  if (suf) {
    FILE *fp = NULL;
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
    fclose(fp);
  } else {
    // can't get a suffix
    ret = -2;
  }
  // write through is ret code 0
 wvret:
  return ret;
}
  
  



// the block size random buffer. Nice ASCII
void generateRandomBufferCyclic(char *buffer, size_t size, unsigned short seedin, size_t cyclic) {

  if (cyclic > size || cyclic == 0) cyclic = size;

  if (cyclic != size) {
    //fprintf(stderr,"*info* generating a random buffer with a size %zd bytes, cyclic %zd bytes\n", size, cyclic);
  }

  unsigned int seed = seedin;
  
  char *user = username();

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
  const size_t topr = sprintf(s, "________________stutools - %s - %u\n", user == NULL ? "" : user, seed);
  strncpy(buffer, s, topr);
  buffer[cyclic-2] = '\n';
  buffer[cyclic-1] = 0;

  for (size_t j = cyclic; j < size; j++) {
    buffer[j] = buffer[j % cyclic];
  }



  free(user);
}


void generateRandomBuffer(char *buffer, size_t size, unsigned short seed) {
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
	return strdup(s);
      }
    } else {
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
  
  int fd = open(filename, O_RDWR | O_CREAT | O_TRUNC | O_DIRECT, S_IRUSR | S_IWUSR);
  if (fd >= 0) {
    fprintf(stderr,"*info* creating '%s' with O_DIRECT, size %zd bytes (%.3g GiB)\n", filename, sz, TOGiB(sz));
  } else {
    fprintf(stderr,"*warning* creating file with O_DIRECT failed (no filesystem support?)\n");
    fprintf(stderr,"*warning* parts of the file will be in the page cache/RAM.\n");
    fd = open(filename, O_RDWR | O_CREAT | O_TRUNC, S_IRUSR | S_IWUSR);
    if (fd < 0) {
      perror(filename);return 1;
    }
  }

  keepRunning = 1;
  if (0) { // fallocate makes the filesystem cheat if you read empty
    int fret = fallocate(fd, FALLOC_FL_ZERO_RANGE, 0, sz);
    if (fret == 0) {
      close(fd);
      fprintf(stderr,"*info* success fallocate\n");
      return 0;
    } else {
      size_t c_sz = fileSizeFromName(filename);
      if (c_sz > 0) {
	fprintf(stderr,"*info* created file size was only %zd (%.2lf GiB)\n", c_sz, TOGiB(c_sz));
	fprintf(stderr,"*info* deleting file '%s'\n", filename);
	remove(filename);
	fprintf(stderr,"*error* exiting.\n");
	exit(-1);
      }
      fprintf(stderr,"*warning* fallocate failed. Creating manually. Error = %d\n", fret);
    }
  }

  char *buf = NULL;
  CALLOC(buf, 1, 1024*1024);
  
  size_t towriteMiB = sz;
  while (towriteMiB > 0 && keepRunning) {
    int towrite = MIN(towriteMiB, 1024*1024);
    int wrote = write(fd, buf, towrite);
    if (wrote < 0) {
      perror("createFile");free(buf);return 1;
    }
    towriteMiB -= towrite;
  }
  fsync(fd);
  close(fd);
  free(buf);
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


int splitRange(const char *charBS, double *low, double *high) {
  if (charBS) {
    
    char *endp = NULL;
    *low = strtod(charBS, &endp);
    *high = *low;
    if (*endp == '-') {
      *high = atof(endp+1);
    }
    return 0;
  } else {
    return 1;
  }
}
