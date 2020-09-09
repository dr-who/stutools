#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <sys/time.h>
#include <limits.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include <malloc.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include <math.h>
#include <sys/sysinfo.h>
#include <sys/utsname.h>
#include <sys/wait.h>
#include <pwd.h>
#include <assert.h>
#include <linux/hdreg.h>
#include <numa.h>
#include <numaif.h>

#include "utils.h"

extern int keepRunning;

/*struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
  }*/

inline double timedouble()
{
  struct timeval now;
  gettimeofday(&now, NULL);
  double tm = ((double)now.tv_sec * 1000000.0) + now.tv_usec;
  assert(tm > 0);
  return tm/1000000.0;
}

size_t fileSize(int fd)
{
  size_t sz = lseek(fd, 0L, SEEK_END);
  lseek(fd, 0L, SEEK_SET);
  return sz;
}

size_t fileSizeFromName(const char *path)
{
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

size_t fileExists(const char *path)
{
  int fd = open(path, O_RDONLY);
  if (fd >= 0) {
    close(fd);
    return 1;
  } else {
    return 0;
  }
}

size_t blockDeviceSizeFromFD(const int fd)
{
  size_t file_size_in_bytes = 0;
  ioctl(fd, BLKGETSIZE64, &file_size_in_bytes);
  return file_size_in_bytes;
}


size_t blockDeviceSize(const char *path)
{

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


size_t swapTotal()
{

  FILE *fp = fopen("/proc/swaps", "rt");
  if (fp == NULL) {
    perror("/proc/swaps");
    return 0;
  }

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


double loadAverage()
{
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


int isBlockDevice(const char *name)
{
  struct stat sb;
  int ret;

  if ((ret = stat(name, &sb)) == -1) {
    //    fprintf(stderr,"*warning* isBlockDevice '%s' returned %d\n", name, ret);
    return 0;
  }

  switch (sb.st_mode & S_IFMT) { // check the filetype bits
  case S_IFBLK:
    ret=1;
    break; //printf("block device\n");            break;
  case S_IFCHR:
    ret=0;
    break; // printf("character device\n");        break;
  case S_IFDIR:
    ret=0;
    break; // printf("directory\n");               break;
  case S_IFIFO:
    ret=0;
    break; // printf("FIFO/pipe\n");               break;
  case S_IFLNK:
    ret=0;
    break; // retprintf("symlink\n");                 break;
  case S_IFREG:
    ret=2;
    break; // printf("regular file\n");            break;
  case S_IFSOCK:
    ret=0;
    break ; //printf("socket\n");                  break;
  default:
    ret=0;
    break; //    printf("unknown?\n");                break;
  }

  return ret;
}

// /sys/devices/system/cpu/cpufreq/policy0/scaling_governor

#define POWERPATH "/sys/devices/system/cpu/cpufreq/policy0/scaling_governor"
void printPowerMode()
{
  FILE *fp = fopen(POWERPATH, "rt");
  if (fp) {
    char s[1000];
    int ret = fscanf(fp, "%s", s);
    if (ret == 1) {
      fprintf(stderr,"*info* power mode: '%s' (%s)\n", s, POWERPATH);
    } else {
      fprintf(stderr,"*info* power mode partial: '%s'\n", s);
    }
    fclose(fp);
  }
}



void dropCaches()
{
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
  fprintf(stderr,"*info* echo 3 > /proc/sys/vm/drop_caches ; # caches dropped\n");
}


char* queueType(char *path)
{
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


size_t numThreads()
{
  return sysconf(_SC_NPROCESSORS_ONLN);
}

size_t totalRAM()
{
  struct sysinfo info;
  sysinfo(&info);
  return info.totalram;
}

size_t getUptime()
{
  struct sysinfo info;
  sysinfo(&info);
  return info.uptime;
}

size_t freeRAM()
{
  struct sysinfo info;
  sysinfo(&info);
  return info.freeram;
}

size_t totalShared()
{
  struct sysinfo info;
  sysinfo(&info);
  return info.sharedram;
}

size_t totalBuffer()
{
  struct sysinfo info;
  sysinfo(&info);
  return info.bufferram;
}

char *OSRelease()
{
  struct utsname buf;
  uname (&buf);
  return strdup(buf.release);
}

int getWriteCacheStatus(int fd)
{
  unsigned long val = 0;
  if (ioctl(fd, HDIO_GET_WCACHE, &val) >= 0) {
    fprintf(stderr,"*info* write cache setting for %d is %lu\n", fd, val);
  } else {
    perror("ioctl");
  }
  return val;
}

int getWriteCache(const char *suf)
{
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



int getRotational(const char *suf)
{
  if (suf == NULL) {
    return 0; // if null then not rotational
  }

  char s[200];
  int rot = 0;
  FILE *fp = NULL;
  if (suf) {
    sprintf(s, "/sys/block/%s/queue/rotational", suf);
    fp = fopen(s, "rt");
    if (!fp) {
      //      perror(s);
      goto wvret;
    }

    if (fscanf(fp, "%d", &rot) == 1) {
      //
    }
  }
wvret:
  if (fp) fclose(fp);
  return rot;
}



char *hostname()
{
  char s[1000];
  gethostname(s, 999);
  if (strchr(s,'.')) {
    *strchr(s,'.') = 0;
  }
  return strdup(s);
}

void checkRandomBuffer4k(const char *buffer, const size_t len) {
  for (size_t i = 4096; i < len; i += 4096) {
    int ret = memcmp(buffer, buffer+i, 4096);
    if (ret) {
      fprintf(stderr,"*error* 4k chunk starting %zd is different from chunk starting at %d (ret %d)\n", i, 0, ret);
    }
  }
}

// the block size random buffer. Nice ASCII
size_t generateRandomBufferCyclic(char *buffer, const size_t size, unsigned short seedin, size_t cyclic)
{
  size_t sumcount = 0;

  assert(cyclic > 0);
  assert(cyclic <= size);
  //  if (cyclic > size || cyclic == 0) cyclic = size;

  //  if (cyclic != size) {
  //    fprintf(stderr,"*info* generating a random buffer with a size %zd bytes, cyclic %zd bytes\n", size, cyclic);
    //  }

  unsigned int seed = seedin;

  const char verystartpoint = ' ' + (rand_r(&seed) % 30);
  const char jump = (rand_r(&seed) % 3) + 1;
  char startpoint = verystartpoint;
  for (size_t j = 0; j < cyclic; j++) {
    buffer[j] = startpoint;
    sumcount += (j ^ startpoint);
    startpoint += jump;
    if (startpoint > 'z') {
      startpoint = verystartpoint;
    }
  }

  /*  char s[1000];
  memset(s, 0, 1000);
  const size_t topr = sprintf(s, "________________stutools - %s - %06d\n", "spit", seedin);
  //  strncpy(buffer, s, topr);
  memcpy(buffer, s, topr);
  */
  
  for (size_t j = cyclic; j < size; j += cyclic) {
    memcpy(buffer + j, buffer, cyclic);
    //    buffer[j] = buffer[j % cyclic];
  }


  {
    //    fprintf(stderr,"%zd\n", cyclic);
    for (size_t j = 0 ; j < size; j+= cyclic) {
      size_t check2 = checksumBuffer(buffer + j, cyclic);
      if (sumcount != check2) {
	fprintf(stderr,"*fatal* memory problem\n");
	exit(1);
      }
    }
  }

  return sumcount;
}



size_t generateRandomBuffer(char *buffer, const size_t size, const unsigned short seed)
{
  //  if (seed == 0) printf("ooon\n");
  return generateRandomBufferCyclic(buffer, size, seed, 4096);
}

size_t checksumBuffer(const char *buffer, const size_t size)
{
  size_t checksum = 0;
  char *p = (char*)buffer;
  for (size_t i = 0; i < size; i++) {
    checksum += (i ^ *p);
    p++;
  }
  return checksum;
}


/* creates a new string */
char *getSuffix(const char *path)
{
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


char *getScheduler(const char *suffix)
{
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



char *getModel(const char *suffix)
{
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


char *getCPUModel()
{
  size_t len = 1000;
  char *s, *retval = NULL;
  CALLOC(s, len, 1);
  
  sprintf(s, "/proc/cpuinfo");
  FILE *fp = fopen(s, "rt");
  int ret = 0;
  if (fp) {
    while ((ret = getline(&s, &len, fp)) > 0) {
      char first[100], sec[100];
      if (sscanf(s, "%s %s", first, sec) >= 2) {
	if ((strcmp(first,"model") == 0) && (strcmp(sec,"name")) == 0) {
	  s[ret - 1] = 0;
	  retval = strdup(strstr(s, ":") + 2);
	  break;
	}
      }
    }
  } else {
    perror(s);
  }
  if (s) free(s);
  return retval;
}


void getPhyLogSizes(const char *suffix, size_t *phy, size_t *max_io_bytes, size_t *log)
{
  *phy = 512;
  *log = 512;
  *max_io_bytes = INT_MAX;
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
        fclose(fp);
        fp = NULL;
      }
    }

    // /sys/block/sda/queue/max_sectors_kb
    sprintf(s, "/sys/block/%s/queue/max_sectors_kb", suffix);
    fp = fopen(s, "rt");
    if (!fp) {
      //      fprintf(stderr,"*error* problem opening %s: returning max_io_bytes 256 KiB\n", s);
      //      *max_io_bytes = 256 * 1024;
    } else {
      //    fprintf(stderr,"opened %s\n", s);
      ret = fscanf(fp, "%d", &d);
      if (ret == 1) {
        *max_io_bytes = d * 1024;
      }
      if (fp) {
        fclose(fp);
        fp = NULL;
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
        fclose(fp);
        fp = NULL;
      }
    }
  }
}


size_t alignedNumber(size_t num, size_t alignment)
{
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
inline size_t randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits, size_t randomValue)
{
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


int canOpenExclusively(const char *fn)
{
  int fd = open(fn, O_RDWR | O_EXCL);
  if (fd < 0) {
    return 0;
  }
  close(fd);
  return 1;
}


size_t canCreateFile(const char *filename, const size_t sz)
{
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


int createFile(const char *filename, const size_t sz)
{
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
      perror(filename);
      return 1;
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
      perror("createFile");
      free(buf);
      return 1;
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

void commaPrint0dp(FILE *fp, double d)
{
  if (d >= 1000) {
    const size_t dd = d;
    d=0;
    if (dd >= 1000000000) {
      fprintf(fp,"%zd,%03zd,%03zd,%03zd", (dd / 1000000000), (dd % 1000000000)/1000000, (dd % 1000000)/1000, dd % 1000);
    } else if (dd >= 1000000) {
      fprintf(fp,"%zd,%03zd,%03zd", (dd/1000000), (dd % 1000000)/1000, dd % 1000);
    } else {
      fprintf(fp,"%3zd,%03zd", dd / 1000, dd % 1000);
    }
  } else {
    fprintf(fp,"%7.0lf", d);
  }
}


// 0 means no values, 1 means one, 2 means two values

int splitRange(const char *charBS, double *low, double *high)
{
  int retvalue = 0;
  if (charBS) {

    char *endp = NULL;
    *low = strtod(charBS, &endp);
    *high = *low;
    retvalue = 1;
    if ((*endp == '-') || (*endp == '_')) {
      *high = atof(endp+1);
      retvalue = 2;
    }
  }
  return retvalue;
}

int splitRangeChar(const char *charBS, double *low, double *high, char *retch)
{
  int retvalue = 0;
  if (charBS) {

    char *endp = NULL;
    *low = strtod(charBS, &endp);
    *high = *low;
    retvalue = 1;
    if ((*endp == '-') || (*endp == '_')) {
      if (retch) {
        *retch = *endp;
      }
      *high = atof(endp+1);
      retvalue = 2;
    }
  }
  return retvalue;
}


size_t dirtyPagesBytes()
{
  long ret = 0;
  FILE *fp = fopen("/proc/meminfo", "rt");
  if (fp) {
    char *line = NULL;
    long temp = 0;
    size_t len = 0;
    ssize_t read = 0;
    char s[100];

    while ((read = getline(&line, &len, fp)) != -1) {
      if (sscanf(line, "%s %ld", s, &temp)==2) {
        if (strcmp(s, "Dirty:")==0) {
          ret = temp * 1024; // stored in KiB
          //	  fprintf(stderr,"*info* dirty %ld bytes\n", ret);
          break;
        }
      }

    }
    fclose(fp);
  }
  return ret;
}


size_t getCachedBytes()
{
  long ret = 0;
  FILE *fp = fopen("/proc/meminfo", "rt");
  if (fp) {
    char *line = NULL;
    long temp = 0;
    size_t len = 0;
    ssize_t read = 0;
    char s[100];

    while ((read = getline(&line, &len, fp)) != -1) {
      if (sscanf(line, "%s %ld", s, &temp)==2) {
        if (strcmp(s, "Cached:")==0) {
          ret = temp * 1024; // stored in KiB
          //	  fprintf(stderr,"*info* dirty %ld bytes\n", ret);
          break;
        }
      }

    }
    fclose(fp);
  }
  return ret;
}

// from: https://www.geeksforgeeks.org/difference-fork-exec/
// examples
//  char* envp[] = { "some", "environment", NULL };
int runCommand(char *program, char *argv_list[])
{
  pid_t  pid;
  int ret = 1;
  int status;
  pid = fork();
  if (pid == -1) {

    // pid == -1 means error occured
    fprintf(stderr,"can't fork, error occured\n");
    exit(EXIT_FAILURE);
  } else if (pid == 0) {

    // pid == 0 means child process created
    // getpid() returns process id of calling process
    //     fprintf(stderr,"child process, pid = %u\n",getpid());

    // the argv list first argument should point to
    // filename associated with file being executed
    // the array pointer must be terminated by NULL
    // pointer

    // the execv() only return if error occured.
    // The return value is -1
    execvp(program, argv_list);
    exit(0);
  } else {
    // a positive number is returned for the pid of
    // parent process
    // getppid() returns process id of parent of
    // calling process
    //     fprintf(stderr,"parent process, pid = %u\n",getppid());

    // the parent process calls waitpid() on the child
    // waitpid() system call suspends execution of
    // calling process until a child specified by pid
    // argument has changed state
    // see wait() man page for all the flags or options
    // used here
    if (waitpid(pid, &status, 0) > 0) {

      //       fprintf(stderr,"%d %d\n", WIFEXITED(status), WEXITSTATUS(status));

      if (WIFEXITED(status) && !WEXITSTATUS(status))
        fprintf(stderr,"*info* program execution successful\n");

      else if (WIFEXITED(status) && WEXITSTATUS(status)) {
        if (WEXITSTATUS(status) == 127) {

          // execv failed
          fprintf(stderr,"execv failed\n");
        } else
          fprintf(stderr,"program terminated normally,"
                  " but returned a non-zero status\n");
      } else
        fprintf(stderr,"program didn't terminate normally\n");
    } else {
      // waitpid() failed
      fprintf(stderr,"waitpid() failed\n");
    }
    //     exit(0);
  }
  return ret;
}

int getNumaCount()
{
  return numa_max_node() + 1;
}

int getNumHardwareThreads()
{
  return numa_num_task_cpus();
}

int cpuCountPerNuma( int numa )
{
  assert( getNumaCount() > 0 );
  assert( numa >= 0 && numa < getNumaCount() );

  struct bitmask* bm = numa_allocate_cpumask();
  numa_node_to_cpus( numa, bm );
  unsigned int cpu_count = numa_bitmask_weight( bm );
  numa_bitmask_free( bm );
  return cpu_count;
}

void getThreadIDs( int numa, int* numa_cpu_list )
{
  assert( getNumaCount() > 0 );
  assert( numa >= 0 && numa < getNumaCount() );
  assert( numa_cpu_list != NULL );

  struct bitmask* bm = numa_allocate_cpumask();
  numa_node_to_cpus( numa, bm );

  size_t cur_list_idx = 0;
  for( int tid = 0; tid < getNumHardwareThreads(); ++tid ) {
    if( numa_bitmask_isbitset( bm, tid ) ) {
      assert( cur_list_idx < (size_t)cpuCountPerNuma( numa ) );
      numa_cpu_list[ cur_list_idx++ ] = tid;
    }
  }

  numa_bitmask_free( bm );
}

int pinThread( pthread_t* thread, int* hw_tids, size_t n_hw_tid )
{
  cpu_set_t cpuset;
  CPU_ZERO( &cpuset );
  for( size_t tid = 0; tid < n_hw_tid; tid++ ) {
    CPU_SET( hw_tids[ tid ], &cpuset );
  }
  int rc = pthread_setaffinity_np( *thread, sizeof( cpu_set_t ), &cpuset );
  return rc;
}


int getDiscardInfo(const char *suffix, size_t *alignment_offset, size_t *discard_max_bytes, size_t *discard_granularity, size_t *discard_zeroes_data) {
  *discard_max_bytes = 0;
  *discard_granularity = 0;
  *discard_zeroes_data = 0;
  *alignment_offset = 0;

  char s[1000];
  sprintf(s, "/sys/block/%s/alignment_offset", suffix);
  FILE *fp = fopen(s, "rt");
  long ret = 0, d = 0;
  if (fp) {
    ret = fscanf(fp, "%ld", &d);
    if (ret == 1) {
      *alignment_offset = d;
    }
    if (fp) {
      fclose(fp);
      fp = NULL;
    }
  }


  sprintf(s, "/sys/block/%s/queue/discard_max_bytes", suffix);
  fp = fopen(s, "rt");
  ret = 0;
  d = 0;
  if (fp) {
    ret = fscanf(fp, "%ld", &d);
    if (ret == 1) {
      *discard_max_bytes = d;
    }
    fclose(fp);
    fp = NULL;
  }

  ret = 0;
  d = 0;
  sprintf(s, "/sys/block/%s/queue/discard_granularity", suffix);
  fp = fopen(s, "rt");
  if (fp) {
    ret = fscanf(fp, "%ld", &d);
    if (ret == 1) {
      *discard_granularity = d;
    }
    fclose(fp);
    fp = NULL;
  }


  ret = 0;
  d = 0;

  sprintf(s, "/sys/block/%s/queue/discard_zeroes_data", suffix);
  fp = fopen(s, "rt");
  if (fp) {
    ret = fscanf(fp, "%ld", &d);
    if (ret == 1) {
      *discard_zeroes_data = d;
    }
    fclose(fp);
    fp = NULL;
  }

  return 0;
}
  

int performDiscard(int fd, const char *path, unsigned long low, unsigned long high, size_t max_bytes, size_t discard_granularity, double *maxdelay_secs, const int verbose) {
  int err = 0;
  double before = 0, delta = 0, start = 0, elapsed = 0;

  if (maxdelay_secs) *maxdelay_secs = 0;

  int calls = ceil(high * 1.0 / max_bytes);
  if (path && verbose) {
    fprintf(stderr,"*info* starting discarding %s, [%.3lf GiB, %.3lf GiB], in %d calls of at most %zd bytes...\n", path, TOGiB(low), TOGiB(high), calls, max_bytes);
  }
  
  if (high - low >= discard_granularity) {

    start = timedouble();
    for (size_t i = low; i < high; i+= max_bytes) {
      
      unsigned long range[2];
      
      range[0] = i;
      range[1] = MIN(high - i, max_bytes);
      
      if (verbose) {
	fprintf(stderr, "*info* sending trim/BLKDISCARD command to %s [%ld, %ld] [%.3lf GiB, %.3lf GiB]\n", path, range[0], range[0] + range[1], TOGiB(range[0]), TOGiB(range[0] + range[1]));
      }
      
      err = 0;
      before = timedouble();
      if ((err = ioctl(fd, BLKDISCARD, &range))) {
	fprintf(stderr, "*error* %s: BLKDISCARD ioctl failed, error = %d\n", path, err);
	perror("trim");
	break;
      }
      delta = timedouble() - before;
      if (maxdelay_secs) {
	if (delta > *maxdelay_secs) {
	  *maxdelay_secs = delta;
	}
      }
    }
    elapsed = timedouble() - start;
    if (path && verbose) {
      fprintf(stderr,"... finished discarding %s. Worse sub-trim delay of %.1lf ms. Total time %.1lf secs\n", path, (maxdelay_secs) ? (*maxdelay_secs) * 1000.0 : 0, elapsed);
    }
  } else {
    err = 1;
    fprintf(stderr,"*error* discard range is too small\n");
  }
    
  
  return err;
}

#include <syslog.h>

void syslogString(const char *prog, const char *message) {
  if (prog) {}
  setlogmask (LOG_UPTO (LOG_NOTICE));

  //  openlog(prog, LOG_CONS | LOG_PID | LOG_NDELAY, LOG_LOCAL1);
  syslog(LOG_NOTICE, message, "");
  //  closelog();
}
