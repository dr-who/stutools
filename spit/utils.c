#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <sys/time.h>
#include <linux/limits.h>
#include <limits.h>
#include <linux/fs.h>
#include <sys/ioctl.h>
#include <sys/file.h>
#include <ctype.h>

#include "linux-headers.h"

#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <math.h>
#include <sys/sysinfo.h>
#include <sys/utsname.h>
#include <sys/wait.h>
#include <pwd.h>
#include <assert.h>
#include <sys/vfs.h>

#include <regex.h>

#include "utils.h"
#include "numList.h"
#include "procDiskStats.h"
#include "diskStats.h"

extern int keepRunning;

/*struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
  }*/

inline double timeAsDouble(void) {
    struct timeval now;
    gettimeofday(&now, NULL);
    double tm = ((double) now.tv_sec * 1000000.0) + now.tv_usec;
    assert(tm > 0);
    return tm / 1000000.0;
}

double timeOnPPS() {
  double thetime = (10*timeAsDouble());
  //  const size_t firsttime = thetime;
  
  size_t loops = 0;
  // first loop until the time doesn't end in .5
  do {
    if (loops) usleep(10); // to avoid doing 20 million time operations
    thetime = (10*timeAsDouble());
    loops++;
  } while ((size_t)thetime%10 == 5);
  // at this point the time doesn't in .5

  do {
    if (loops) usleep(10); // to avoid doing 20 million time operations
    thetime = (10*timeAsDouble());
    loops++;
  } while ((size_t)thetime%10 != 5);

  return thetime / 10.0;
}

size_t fileSize(int fd) {
    size_t sz = lseek(fd, 0L, SEEK_END);
    lseek(fd, 0L, SEEK_SET);
    return sz;
}

size_t fileSizeFromName(const char *path) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
      perror(path);
        //    fprintf(stderr,"*info* no file present '%s'\n", path);
      return 0;
    }
    size_t sz = lseek(fd, 0L, SEEK_END);
    size_t ret = lseek(fd, 0L, SEEK_SET);
    unsigned int major, minor;
    majorAndMinor(fd, &major, &minor);
    close(fd);
    if (sz == 0) {
      // may
      if (ret == 0) {
	// zero but can set seek
	// might be /dev/null
	if ((major==1) && (minor==3)) {
	  fprintf(stderr, "*info* device is /dev/null, pretending it's 1EB in size\n");
	  sz = (size_t)(1000L*1000*1000*1000*1000*1000);
	}
      }
    }
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
        //    fprintf(stderr,"*warning*: block device '%s' has a size of %d bytes.\n", path, 0);
        file_size_in_bytes = 1; // make it 1 to avoid DBZ
    }
    return file_size_in_bytes;
}


double swapTotal(size_t *t, size_t *u) {

    *t = 0;
    *u = 0;

    FILE *fp = fopen("/proc/swaps", "rt");
    if (fp == NULL) {
        perror("/proc/swaps");
        return 0;
    }

    char *line = NULL;
    size_t len = 0;
    ssize_t read = 0;

    while ((read = getline(&line, &len, fp)) != -1) {
        if (line[0] == '/') {
            // a /dev line
	    size_t size, used;
            char name[NAME_MAX], part[NAME_MAX];
            int s = sscanf(line, "%s %s %zu %zu", name, part, &size, &used);
            if (s == 4) {
                // in /proc the size is in KiB
	      (*t) = (*t) + (size << 10);
	      (*u) = (*u) + (used << 10);
            }
        }
    }

    free(line);
    fclose(fp);

    return (*u) * 100.0 / (*t);
}


void loadAverage3(double *d1, double *d2, double *d3) {
    FILE *fp = fopen("/proc/loadavg", "rt");
    if (fp == NULL) {
        perror("can't open /proc/loadavg");
        return;
    }
    if (fscanf(fp, "%lf %lf %lf", d1, d2, d3) != 3) {
        fprintf(stderr, "warning: problem with loadavg\n");
    }
    fclose(fp);
}

double loadAverage(void) {
    FILE *fp = fopen("/proc/loadavg", "rt");
    if (fp == NULL) {
        perror("can't open /proc/loadavg");
        return 0;
    }
    double loadavg = 0;
    if (fscanf(fp, "%lf", &loadavg) != 1) {
        fprintf(stderr, "warning: problem with loadavg\n");
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
        case S_IFBLK:
            ret = 1;
            break; //printf("block device\n");            break;
        case S_IFCHR:
            ret = 0;
            break; // printf("character device\n");        break;
        case S_IFDIR:
            ret = 0;
            break; // printf("directory\n");               break;
        case S_IFIFO:
            ret = 0;
            break; // printf("FIFO/pipe\n");               break;
        case S_IFLNK:
            ret = 0;
            break; // retprintf("symlink\n");                 break;
        case S_IFREG:
            ret = 2;
            break; // printf("regular file\n");            break;
        case S_IFSOCK:
            ret = 0;
            break; //printf("socket\n");                  break;
        default:
            ret = 0;
            break; //    printf("unknown?\n");                break;
    }

    return ret;
}

// /sys/devices/system/cpu/cpufreq/policy0/scaling_governor

#define POWERPATH "/sys/devices/system/cpu/cpufreq/policy0/scaling_governor"


char *getPowerMode(void) {
    FILE *fp = fopen(POWERPATH, "rt");
    char *res = NULL;
    if (fp) {
        char s[NAME_MAX];
        int ret = fscanf(fp, "%s", s);
        if (ret == 1) {
            res = strdup(s);
        }
        fclose(fp);
    }
    if (res == NULL) {
        res = strdup("");
    }
    return res;
}


void printPowerMode(void) {
    char *res = getPowerMode();
    if (res) {
        fprintf(stderr, "*info* power mode: '%s' (%s)\n", res, POWERPATH);
        free(res);
    }
}


void dropCaches(void) {
    FILE *fp = fopen("/proc/sys/vm/drop_caches", "wt");
    if (fp == NULL) {
        fprintf(stderr, "**error** you need sudo/root permission to drop caches\n");
	return;
    }
    if (fprintf(fp, "3\n") < 1) {
        fprintf(stderr, "**error** you need sudo/root permission to drop caches\n");
	return;
    }
    fflush(fp);
    fclose(fp);
    fprintf(stderr, "*info* echo 3 > /proc/sys/vm/drop_caches ; # caches dropped\n");
}


char *queueType(const char *path) {
    if (path) {
    }
    FILE *fp = fopen("/sys/block/sda/device/queue_type", "rt");
    if (fp == NULL) {
        perror("problem opening");
        //    exit(-1);
    }
    char instr[NAME_MAX];
    size_t r = fscanf(fp, "%s", instr);
    fclose(fp);
    if (r != 1) {
        return strdup("n/a");
        //    fprintf(stderr,"error: problem reading from '%s'\n", path);
        //    exit(-1);
    }
    return strdup(instr);
}


size_t numThreads(void) {
    return sysconf(_SC_NPROCESSORS_ONLN);
}

size_t totalRAM(void) {
    struct sysinfo info;
    sysinfo(&info);
    return info.totalram;
}

size_t getUptime(void) {
    struct sysinfo info;
    sysinfo(&info);
    return info.uptime;
}

size_t freeRAM(void) {
    struct sysinfo info;
    sysinfo(&info);
    return info.freeram;
}

size_t totalShared(void) {
    struct sysinfo info;
    sysinfo(&info);
    return info.sharedram;
}

size_t totalBuffer(void) {
    struct sysinfo info;
    sysinfo(&info);
    return info.bufferram;
}

char *OSRelease(void) {
    struct utsname buf;
    uname(&buf);
    return strdup(buf.release);
}

int getWriteCacheStatus(int fd) {
    unsigned long val = 0;
    if (ioctl(fd, HDIO_GET_WCACHE, &val) >= 0) {
        fprintf(stderr, "*info* write cache setting for %d is %lu\n", fd, val);
    } else {
        perror("ioctl");
    }
    return val;
}

int getWriteCache(const char *suf) {
    char s[PATH_MAX], s2[PATH_MAX];
    int ret = 0;
    FILE *fp = NULL;
    if (suf) {
        sprintf(s, "/sys/block/%s/queue/write_cache", suf);
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

int getNumRequests(const char *suf) {
    if (suf == NULL) {
        return 0; // if null then not rotational
    }

    char s[PATH_MAX];
    int nr = 0;
    FILE *fp = NULL;
    if (suf) {
        sprintf(s, "/sys/block/%s/queue/nr_requests", suf);
        fp = fopen(s, "rt");
        if (!fp) {
            //      perror(s);
            goto wvret;
        }

        if (fscanf(fp, "%d", &nr) == 1) {
            //
        }
    }
    wvret:
    if (fp) fclose(fp);
    return nr;
}


int getRotational(const char *suf) {
    if (suf == NULL) {
        return 0; // if null then not rotational
    }

    char s[PATH_MAX];
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


char *hostname(void) {
    char s[NAME_MAX];
    gethostname(s, NAME_MAX - 1);
    if (strchr(s, '.')) {
        *strchr(s, '.') = 0;
    }
    return strdup(s);
}

void checkRandomBuffer4k(const char *buffer, const size_t len) {
    for (size_t i = 4096; i < len; i += 4096) {
        int ret = memcmp(buffer, buffer + i, 4096);
        if (ret) {
            fprintf(stderr, "*error* 4k chunk starting %zd is different from chunk starting at %d (ret %d)\n", i, 0,
                    ret);
        }
    }
}

// the block size random buffer. Nice ASCII
size_t generateRandomBufferCyclic(char *buffer, const size_t size, unsigned short seedin, size_t cyclic) {
    size_t sumcount = 0;

    //  if (cyclic > size || cyclic == 0) cyclic = size;

    //  if (cyclic != size) {
    //    fprintf(stderr,"*info* generating a random buffer with a size %zd bytes, cyclic %zd bytes\n", size, cyclic);
    //  }

    assert(cyclic > 0);
    //  assert(cyclic <= size);
    if (cyclic > size) cyclic = size;


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

    for (size_t j = cyclic; j < size; j += cyclic) {
        memcpy(buffer + j, buffer, cyclic);
        //    buffer[j] = buffer[j % cyclic];
    }


    {
        //    fprintf(stderr,"%zd\n", cyclic);
        for (size_t j = 0; j < size; j += cyclic) {
            size_t check2 = checksumBuffer(buffer + j, cyclic);
            if (sumcount != check2) {
                fprintf(stderr, "*fatal* memory problem\n");
                exit(1);
            }
        }
    }

    return sumcount;
}


size_t generateRandomBuffer(char *buffer, const size_t size, const unsigned short seed) {
    //  if (seed == 0) printf("ooon\n");
    //  fprintf(stderr,"len %zd\n", size);
    return generateRandomBufferCyclic(buffer, size, seed, 4096);
}

size_t checksumBuffer(const char *buffer, const size_t size) {
    size_t checksum = 0;
    char *p = (char *) buffer;
    for (size_t i = 0; i < size; i++) {
        checksum += (i ^ *p);
        p++;
    }
    return checksum;
}


/* creates a new string */
char *getSuffix(const char *path) {
    if (!path) return NULL;
    int found = -1;
    for (size_t i = strlen(path) - 1; i > 0; i--) {
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
        char s[NAME_MAX];
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

            for (size_t i = 0; i < strlen(s); i++) {
                if (s[i] == ' ') s[i] = '_';
            }
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


char *getCPUModel(void) {
    size_t len = 1000;
    char *s, *retval = NULL;
    CALLOC(s, len, 1);

    sprintf(s, "/proc/cpuinfo");
    FILE *fp = fopen(s, "rt");
    int ret = 0;
    if (fp) {
        while ((ret = getline(&s, &len, fp)) > 0) {
            char first[NAME_MAX], sec[NAME_MAX];
            if (sscanf(s, "%s %s", first, sec) >= 2) {
                if ((strcmp(first, "model") == 0) && (strcmp(sec, "name")) == 0) {
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


void getPhyLogSizes(const char *suffix, size_t *phy, size_t *max_io_bytes, size_t *log) {
    *phy = 512;
    *log = 512;
    *max_io_bytes = INT_MAX;
    if (suffix) {
        char s[PATH_MAX];
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


size_t alignedNumber(size_t num, size_t alignment) {
    size_t ret = num;
    if (alignment) {

        ret = num / alignment;
        //    fprintf(stderr,"num %zd / %zd = ret %zd\n", num, alignment, ret);
        if (num % alignment > num / 2) {
            ret++;
        }
        ret = ret * alignment;

        //      fprintf(stderr,"requested %zd returned %zd\n", num, ret);
    }

    return ret;
}

// return a random size, between [lowbsBytes and highbsBytes] aligned at alignmentbits, randomvalue in [low,high] range
// both low and high are inclusive, so those values are allowed.
// alignments bits is as it says, the shift left/right powers of 2 bits.
inline size_t
randomBlockSize(const size_t lowbsBytes, const size_t highbsBytes, const size_t alignmentbits, const size_t randomValue) {
    if (highbsBytes == 0) {
        return 0;
    }
    if (lowbsBytes > highbsBytes) {
      fprintf(stderr, "*error* lowbs %zd > highbs %zd\n", lowbsBytes, highbsBytes);
    }
    
    //    assert(alignmentbits < 100);

    size_t lowbs_k = lowbsBytes >> alignmentbits; // 1 / 4096 = 0
    //  if (lowbs_k < 1) lowbs_k = 1;
    size_t highbs_k = highbsBytes >> alignmentbits;   // 8 / 4096 = 2
    //  if (highbs_k < 1) highbs_k = 1;

    if ((randomValue >> alignmentbits) > highbs_k) {
      fprintf(stderr, "*error* randomBlockSize out of range\n");
    }

    size_t randombs_k = lowbs_k;
    if (highbs_k > lowbs_k) {
      randombs_k += (randomValue >>alignmentbits);
    }

    size_t randombs = randombs_k << alignmentbits;

    assert(randombs >= lowbsBytes);
    assert(randombs <= highbsBytes);
    //  fprintf(stderr,"random bytes %zd\n", randombs);
    return randombs;
}

int startsWith(const char *pre, const char *str) {
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


/*size_t canCreateFile(const char *filename, const size_t sz)
{
  fprintf(stderr,"*info* can create '%s', size %zd bytes (%.3g GiB)\n", filename, sz, TOGiB(sz));
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
  }*/


int createFile(const char *filename, const size_t sz) {
    if (!filename) {
        fprintf(stderr, "*error* no filename\n");
        exit(1);
    }
    assert(sz);

    if (fileSizeFromName(filename) == sz) { // already there and right size
        fprintf(stderr, "*info* file %s already exists with size %zd\n", filename, sz);
        return 0;
    }
    fprintf(stderr, "*info* create file '%s' size %zd\n", filename, sz);


    if (startsWith("/dev/", filename)) {
        fprintf(stderr, "*error* path error, will not create a file '%s' in the directory /dev/\n", filename);
        exit(-1);
    }

    /*  size_t create_sz = canCreateFile(filename, sz);
    if (create_sz) { // if it fallocated any file at all
      if (create_sz != sz) { // and if it's wrong size
        fprintf(stderr,"*error* can't create filename '%s', limited to size %zd (%.1lf GiB)\n", filename, create_sz, TOGiB(create_sz));
        exit(-1);
      }
      }*/

    double timestart = timeAsDouble();
    // open with O_DIRECT to avoid filling page cache then only to flush it
    int fd = open(filename, O_RDWR | O_CREAT | O_TRUNC | O_DIRECT, S_IRUSR | S_IWUSR);
    if (fd < 0) {
        fprintf(stderr, "*warning* creating file with O_DIRECT failed (no filesystem support?)\n");
        fprintf(stderr, "*warning* parts of the file will be in the page cache/RAM.\n");
        fd = open(filename, O_RDWR | O_CREAT | O_TRUNC, S_IRUSR | S_IWUSR);
        if (fd < 0) {
            perror(filename);
            exit(1);
            //      return 1;
        }
    }
    int fdret = fallocate(fd, 0, 0, sz);
    if (fdret == 0) {
        fprintf(stderr, "*info* fallocate '%s' succeeded with length = %zd\n", filename, sz);
    } else {
        perror("*info* fallocate potentially not supported: ");
    }


    keepRunning = 1;
    char *buf = NULL;

#define CREATECHUNK (1024*1024*100L)

    CALLOC(buf, 1, CREATECHUNK);
    generateRandomBuffer(buf, CREATECHUNK, 42);
    fprintf(stderr, "*info* slow write(fd, %ld, buf) with '%s' %zd (%.3lf GiB)\n", CREATECHUNK, filename, sz,
            TOGiB(sz));
    size_t towriteMiB = sz;
    //totalw = 0;

    float percent = 1.0;
    while ((towriteMiB > 0) && keepRunning) {
        int towrite = MIN(towriteMiB, CREATECHUNK);
        int wrote = write(fd, buf, towrite);
        // if (towrite < sz * percent) {
        //fprintf(stderr,"%.0f%% ", 100-(percent*100)); fflush(stderr);
        //}

        percent = percent - 0.05;
        if (percent < 0) percent = 0;

        if (wrote > 0) {
            // totalw += wrote;
            towriteMiB -= wrote;
        } else {
            if (wrote < 0) {
                perror("createFile");
                free(buf);
                exit(1);
                return 1;
            }
        }
    }
    fflush(stderr);
    fsync(fd);
    fdatasync(fd);
    close(fd);
    free(buf);
    buf = NULL;
    fprintf(stderr, "*info* wrote down to %zd / %zd\n", towriteMiB, sz);
    double timeelapsed = timeAsDouble() - timestart;
    fprintf(stderr, "*info* file '%s' created in %.1lf seconds, %.0lf MB/s\n", filename, timeelapsed,
            TOMB(sz / timeelapsed));

    if (fileSizeFromName(filename) != sz) {
        fprintf(stderr, "*error* file size incorrect\n");
    }
    if (!keepRunning) {
        fprintf(stderr, "*warning* early size creation termination\n");
        exit(-1);
    }
    return 0;
}

void commaPrint0dp(FILE *fp, double d) {
    if (d >= 1000) {
        const size_t dd = d;
        d = 0;
        if (dd >= 1000000000) {
            fprintf(fp, "%zd,%03zd,%03zd,%03zd", (dd / 1000000000), (dd % 1000000000) / 1000000, (dd % 1000000) / 1000,
                    dd % 1000);
        } else if (dd >= 1000000) {
            fprintf(fp, "%zd,%03zd,%03zd", (dd / 1000000), (dd % 1000000) / 1000, dd % 1000);
        } else {
            fprintf(fp, "%3zd,%03zd", dd / 1000, dd % 1000);
        }
    } else {
        fprintf(fp, "%7.0lf", d);
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
        if ((*endp == '-') || (*endp == '_')) {
            *high = atof(endp + 1);
            retvalue = 2;
        }
    }
    return retvalue;
}

int splitRangeChar(const char *charBS, double *low, double *high, char *retch) {
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
            *high = atof(endp + 1);
            retvalue = 2;
        }
    }
    return retvalue;
}


size_t dirtyPagesBytes(void) {
    long ret = 0;
    FILE *fp = fopen("/proc/meminfo", "rt");
    if (fp) {
        char *line = NULL;
        long temp = 0;
        size_t len = 0;
        ssize_t read = 0;
        char s[NAME_MAX];

        while ((read = getline(&line, &len, fp)) != -1) {
            if (sscanf(line, "%s %ld", s, &temp) == 2) {
                if (strcmp(s, "Dirty:") == 0) {
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


size_t getCachedBytes(void) {
    long ret = 0;
    FILE *fp = fopen("/proc/meminfo", "rt");
    if (fp) {
        char *line = NULL;
        long temp = 0;
        size_t len = 0;
        ssize_t read = 0;
        char s[NAME_MAX];

        while ((read = getline(&line, &len, fp)) != -1) {
            if (sscanf(line, "%s %ld", s, &temp) == 2) {
                if (strcmp(s, "Cached:") == 0) {
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
int runCommand(char *program, char *argv_list[]) {
    pid_t pid;
    int ret = 1;
    int status;
    pid = fork();
    if (pid == -1) {

        // pid == -1 means error occured
        fprintf(stderr, "can't fork, error occured\n");
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
                fprintf(stderr, "*info* program execution successful\n");

            else if (WIFEXITED(status) && WEXITSTATUS(status)) {
                if (WEXITSTATUS(status) == 127) {

                    // execv failed
                    fprintf(stderr, "execv failed\n");
                } else
                    fprintf(stderr, "program terminated normally,"
                                    " but returned a non-zero status\n");
            } else
                fprintf(stderr, "program didn't terminate normally\n");
        } else {
            // waitpid() failed
            fprintf(stderr, "waitpid() failed\n");
        }
        //     exit(0);
    }
    return ret;
}

int getNumaCount(void) {
    return numa_max_node() + 1;
}

int getNumHardwareThreads(void) {
    return numa_num_task_cpus();
}

int cpuCountPerNuma(int numa) {
    /*  assert( getNumaCount() > 0 );
        assert( numa >= 0 && numa < getNumaCount() );*/

    struct bitmask *bm = numa_allocate_cpumask();
    numa_node_to_cpus(numa, bm);

    unsigned int cpu_count = numa_bitmask_weight(bm);
    numa_bitmask_free(bm);
    //  numa_free_cpumask(bm);

    return cpu_count;
}

void getThreadIDs(int numa, int *numa_cpu_list) {
    assert(getNumaCount() > 0);
    assert(numa >= 0 && numa < getNumaCount());
    assert(numa_cpu_list != NULL);

    const size_t ccpn = cpuCountPerNuma(numa);

    struct bitmask *bm = numa_allocate_cpumask();
    numa_node_to_cpus(numa, bm);

    size_t cur_list_idx = 0;
    for (int tid = 0; tid < getNumHardwareThreads(); ++tid) {
        if (numa_bitmask_isbitset(bm, tid)) {
            assert(cur_list_idx < ccpn);
            numa_cpu_list[cur_list_idx++] = tid;
        }
    }

    numa_bitmask_free(bm);
}

int pinThread(pthread_t *thread, int *hw_tids, size_t n_hw_tid) {
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    for (size_t tid = 0; tid < n_hw_tid; tid++) {
        CPU_SET(hw_tids[tid], &cpuset);
    }
    int rc = pthread_setaffinity_np(*thread, sizeof(cpu_set_t), &cpuset);
    return rc;
}

void getBaseBlockDevice(const char *block_device, char *base_block_device) {
    if (isBlockDevice(block_device) == 1) {
        char cmd[128];
        FILE *fp;
        int ret = -1;
        sprintf(cmd, "lsblk -ndo pkname /dev/%s", block_device);
        fp = popen(cmd, "r");
        if (fp) {
            ret = fscanf(fp, "%s", base_block_device);
            pclose(fp);
        }
        if (ret == -1)
            base_block_device = (char *) block_device;
    }
}

int getDiscardInfo(const char *suffix, size_t *alignment_offset, size_t *discard_max_bytes, size_t *discard_granularity,
                   size_t *discard_zeroes_data) {
    *discard_max_bytes = 0;
    *discard_granularity = 0;
    *discard_zeroes_data = 0;
    *alignment_offset = 0;

    // if empty then make sure we clear results
    if (suffix == NULL) {
        return 0;
    }

    char base_block_device[156];
    base_block_device[0] = 0;
    if (strchr(base_block_device, '/')) {
        // if we have a / find the base
        getBaseBlockDevice(suffix, base_block_device);
    } else {
        // otherwise use it as is
        strcpy(base_block_device, suffix);
    }

    char s[PATH_MAX];
    sprintf(s, "/sys/block/%s/alignment_offset", base_block_device);
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


    sprintf(s, "/sys/block/%s/queue/discard_max_bytes", base_block_device);
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
    sprintf(s, "/sys/block/%s/queue/discard_granularity", base_block_device);
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

    sprintf(s, "/sys/block/%s/queue/discard_zeroes_data", base_block_device);
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


int performDiscard(int fd, const char *path, unsigned long low, unsigned long high, size_t max_bytes,
                   size_t discard_granularity, double *maxdelay_secs, const int verbose, int zeroall) {
    int err = 0;
    double before = 0, delta = 0, start = 0, elapsed = 0;

    if (maxdelay_secs) *maxdelay_secs = 0;

    int calls;
    if (max_bytes) {
        calls = ceil((high * 1.0 - low) / max_bytes);
    } else {
        calls = 1;
        max_bytes = high - low;
    }

    if (path) {
        fprintf(stderr, "*info* starting discarding %s, [%.3lf GiB, %.3lf GiB], in %d calls of at most %zd bytes...\n",
                path, TOGiB(low), TOGiB(high), calls, max_bytes);
    }


    if (high - low >= discard_granularity) {

        start = timeAsDouble();
        for (size_t i = low; i < high; i += max_bytes) {

            unsigned long range[2];

            range[0] = i;
            range[1] = MIN(high - i, max_bytes);

            if (verbose) {
                fprintf(stderr, "*info* sending trim/BLKDISCARD command to %s [%ld, %ld] [%.3lf GiB, %.3lf GiB]\n",
                        path, range[0], range[0] + range[1], TOGiB(range[0]), TOGiB(range[0] + range[1]));
            }

            err = 0;
            before = timeAsDouble();

            //      fprintf(stderr,"err %lu %lu\n", range[0], range[1]);
            err = ioctl(fd, BLKDISCARD, &range);

            if (zeroall) {
                //fprintf(stderr, "*error* %s: BLKDISCARD ioctl failed, error = %d\n", path, err);
                //perform using a dd?
                unsigned long maxzero = 128 * 1024 * 1024;
                if (range[1] < maxzero) {
                    maxzero = range[1];
                } else {
                    fprintf(stderr, "*info* truncating the zero region to be %zd bytes\n", maxzero);
                }
                char *trimdata;
                CALLOC(trimdata, maxzero, sizeof(char));
                memset(trimdata, 'z', maxzero);
                assert(trimdata);

                //	fprintf(stderr,"%lu %ld\n", range[1], i);
                long unsigned t = pwrite(fd, trimdata, maxzero, i);
                fsync(fd);
                memset(trimdata, 'r', maxzero);

                if (t != maxzero) {
                    perror("trim");
                } else {
                    fprintf(stderr, "*info* '%s' wrote %zd bytes data at position %zd\n", path, maxzero, i);
                }

                t = pread(fd, trimdata, maxzero, i);

                for (size_t tt = 0; tt < maxzero; tt++) {
                    if (trimdata[tt] != 'z') {
                        fprintf(stderr, "*error* didn't erase the region on '%s'\n", path);
                        break;
                    }
                }

                free(trimdata);

                break;
            } else {
                if (err) {
                    fprintf(stderr, "*warning* device '%s' doesn't support TRIM\n", path);
                }
            }
            delta = timeAsDouble() - before;
            if (maxdelay_secs) {
                if (delta > *maxdelay_secs) {
                    *maxdelay_secs = delta;
                }
            }
        }
        elapsed = timeAsDouble() - start;
        if (path) {
            fprintf(stderr, "... finished discarding %s. Worse sub-trim delay of %.1lf ms. Total time %.1lf secs\n",
                    path, (maxdelay_secs) ? (*maxdelay_secs) * 1000.0 : 0, elapsed);
        }
    } else {
        err = 1;
        fprintf(stderr, "*error* discard range is too small\n");
    }

    return err;
}

#include <syslog.h>


void sysLogArgs(const char *prog, int argc, char *argv[]) {
    if (argc < 2)
        return;

    char *s = malloc(10000);
    assert(s);
    char *orig = s;
    s += sprintf(s, "%s: ", prog);
    for (int i = 0; i < argc; i++) {
        s += sprintf(s, "%s ", argv[i]);
	if ((s - orig)>9000) // don't get too close
	  break;
    }
    //  fprintf(stderr,"log: %s\n", orig);
    syslog(LOG_NOTICE, orig, "");

    FILE *fp = fopen("/dev/kmsg", "at");
    if (fp) {
        fprintf(fp, "%s\n", orig);
        fclose(fp);
    }

    free(orig);
}


void syslogString(const char *prog, const char *message) {
    if (prog) {}
    setlogmask(LOG_UPTO (LOG_NOTICE));

    //  openlog(prog, LOG_CONS | LOG_PID | LOG_NDELAY, LOG_LOCAL1);
    if (message && (strlen(message)<1023)) {
      char s[1024];
      sprintf(s, "%s - %s", ttyname(1)?ttyname(1):"-", message?message:"");
      syslog(LOG_NOTICE, s, "");
    } else if (message == NULL) {
      syslog(LOG_NOTICE, "msg empty");
      abort();
    } else {
      syslog(LOG_NOTICE, "msg not valid");
      fprintf(stderr,"*warning* msg not valid: '%s'\n", message);
    }
    //  closelog();
}


int endsWith(const char *str, const char *suffix) {
    if (!str || !suffix) {
        return 0;
    }
    size_t lenstr = strlen(str);
    size_t lensuffix = strlen(suffix);
    if (lensuffix > lenstr) {
        return 0;
    }
    return strncasecmp(str + lenstr - lensuffix, suffix, lensuffix) == 0;
}


// if not suffix, multiple by defaultScale
// if o then ordinary number * defaultScale
// if "per" then percent
size_t stringToBytesDefault(const char *str, const size_t defaultScale, const size_t fullsize) {
    size_t ret = 0;
    assert(defaultScale > 0);
    if (str && (strlen(str) >= 1)) {
        if (endsWith(str, "per") || endsWith(str, "per")) { // return % of default scale
	    ret = (atof(str) / 100.0) * fullsize;
	} else if (endsWith(str, "E")) {
            fprintf(stderr, "*warning* '%s' assuming EB. Must be {K,M,G,T,P,E}[i*]B\n", str);
            ret = 1000L * atof(str) * 1000 * 1000 * 1000 * 1000 * 1000;
	} else if (endsWith(str, "P")) {
            fprintf(stderr, "*warning* '%s' assuming PB. Must be {K,M,G,T,P,E}[i*]B\n", str);
            ret = 1000L * atof(str) * 1000 * 1000 * 1000 * 1000;
        } else if (endsWith(str, "T")) {
            fprintf(stderr, "*warning* '%s' assuming TB. Must be {K,M,G,T,P,E}[i*]B\n", str);
            ret = 1000L * atof(str) * 1000 * 1000 * 1000;
        } else if (endsWith(str, "G")) {
            fprintf(stderr, "*warning* '%s' assuming GB. Must be {K,M,G,T,P,E}[i*]B\n", str);
            ret = 1000L * atof(str) * 1000 * 1000;
        } else if (endsWith(str, "M")) {
            fprintf(stderr, "*warning* '%s' assuming MB. Must be {K,M,G,T,P,E}[i*]B\n", str);
            ret = 1000L * atof(str) * 1000;
        } else if (endsWith(str, "K")) {
            fprintf(stderr, "*warning* '%s' assuming KB. Must be {K,M,G,T,P,E}[i*]B\n", str);
            ret = 1000L * atof(str);
        } else if (endsWith(str, "EiB")) {
	    ret = 1024L * atof(str) * 1024 * 1024 * 1024 * 1024 * 1024;
        } else if (endsWith(str, "PiB")) {
            ret = 1024L * atof(str) * 1024 * 1024 * 1024 * 1024;
        } else if (endsWith(str, "TiB")) {
            ret = 1024L * atof(str) * 1024 * 1024 * 1024;
        } else if (endsWith(str, "GiB")) {
            ret = 1024L * atof(str) * 1024 * 1024;
        } else if (endsWith(str, "MiB")) {
            ret = 1024L * atof(str) * 1024;
        } else if (endsWith(str, "KiB")) {
            ret = 1024L * atof(str);
        } else if (endsWith(str, "EB")) {
	    ret = 1000L * atof(str) * 1000 * 1000 * 1000 * 1000 * 1000;
        } else if (endsWith(str, "PB")) {
            ret = 1000L * atof(str) * 1000 * 1000 * 1000 * 1000;
        } else if (endsWith(str, "TB")) {
            ret = 1000L * atof(str) * 1000 * 1000 * 1000;
        } else if (endsWith(str, "GB")) {
            ret = 1000L * atof(str) * 1000 * 1000;
        } else if (endsWith(str, "MB")) {
            ret = 1000L * atof(str) * 1000;
        } else if (endsWith(str, "KB")) {
            ret = 1000L * atof(str);
        } else if (endsWith(str, "B")) {
            ret = atof(str);
        } else if (endsWith(str, "o") || endsWith(str, "O")) {
	  ret = atof(str) * fullsize;
        } else {
	    ret = atof(str) * defaultScale;
            /*if (assumePow2) {
                // no known or ambiguous suffix, so GiB
                ret = 1024L * atof(str) * 1024 * 1024;
            } else {
                ret = 1000L * atof(str) * 1000 * 1000;
		}*/
        }
    }

    return ret;
}






// if -4 then 4 byte numbers
// if -8 then 8 byte numbers

double entropyTotalBits(unsigned char *buffer, size_t size, int bytes) {
    const size_t bits = 8 * bytes;
    size_t counts0[bits], counts1[bits], tot[bits];

    for (size_t i = 0; i < bits; i++) {
        counts0[i] = 0;
        counts1[i] = 0;
        tot[i] = 0;
    }

    size_t sz = 0;
    for (size_t i = 0; i < size; i++) {
        unsigned long thev;
        if (bytes == 1) {
            unsigned char *v = &buffer[i];
            thev = (int) (*v);
        } else if (bytes == 4) {
            unsigned int *v = (unsigned int *) &buffer[i];
            thev = (unsigned int) (*v);
        } else if (bytes == 8) {
            unsigned long *v = (unsigned long *) &buffer[i];
            thev = (unsigned long) (*v);
        } else if (bytes == 2) {
            unsigned short *v = (unsigned short *) &buffer[i];
            thev = (unsigned short) (*v);
        } else {
            abort();
        }

        //    fprintf(stderr, "%lx\n", thev);
        for (size_t j = 0; j < bits; j++) {
            if (thev & (1L << j)) {
                counts1[j]++;
            } else {
                counts0[j]++;
            }
            tot[j]++;
        }
    }

    sz = 0;
    for (size_t i = 0; i < bits; i++) {
        sz += tot[i];
    }


    double entropy = NAN;

    if (sz) {
        entropy = 0;
        //    double bps = 0;
        //    fprintf(stderr,"size: %ld\n", sz);
        for (size_t i = 0; i < bits; i++) {
            if (counts0[i]) {
                double e = counts0[i] * log((counts0[i] * 1.0 / tot[i])) / log(2.0);
                //	if (verbose) fprintf(stderr,"[b%02zd=0] %3zd %3zd %.4lf\n", i, counts0[i], tot[i], e);
                entropy = entropy - e;
            }

            if (counts1[i]) {
                double e = counts1[i] * log((counts1[i] * 1.0 / tot[i])) / log(2.0);
                //	if (verbose) fprintf(stderr,"[b%02zd=1] %3zd %3zd %.4lf\n", i, counts1[i], tot[i], e);
                entropy = entropy - e;
            }
        }
        //    bps = bits * entropy / sz;
        //    const double threshold = bits * 0.99;
        //    fprintf(stderr,"entropy %.4lf, %zd\n", entropy, bits);
        //    fprintf(stdout, "%.7lf bps (compression %.1lfx) %s\n", bps, bits/bps, bps >= threshold ? "RANDOM" : "");
    }

    return entropy;
}


double entropyTotalBytes(unsigned char *buffer, size_t size) {
    size_t counts[256];

    for (size_t i = 0; i < 256; i++) {
        counts[i] = 0;
    }

    size_t sz = 0;
    for (size_t i = 0; i < size; i++) {
        unsigned int v = (unsigned int) buffer[i];
        assert (v < 256);
        counts[v]++;
    }
    sz += size;

    double entropy = NAN;

    if (sz) {
        entropy = 0;
        //                fprintf(stderr,"size: %ld\n", sz);
        for (size_t i = 0; i < 256; i++) {
            if (counts[i]) {
                double e = log(counts[i] * 1.0 / sz) / log(2.0);
                entropy = entropy - (counts[i] * e);
            }
        }
    }
    return entropy;
}


int openRunLock(const char *fn) {
    int fd = open(fn, O_RDWR | O_CREAT, S_IRUSR | S_IWUSR);
    if (fd > 0) {
        int fl = flock(fd, LOCK_EX | LOCK_NB);
        if (fl < 0) {
            exit(2); // another one is running
        }
        // open therefore we can run
        return 1;
    }
    perror(fn);
    return 0;
}


// if file is called x,
// mv x.2 to x.3, mv x.1 to x.2, mv x to x.1;
int backupExistingFile(const char *file, int versions) {
    char old[PATH_MAX], new[PATH_MAX];

    for (size_t v = versions; v >= 1; v--) {

        if (v == 1) {
            sprintf(old, "%s", file);
        } else {
            sprintf(old, "%s-%zd", file, v - 1);
        }

        sprintf(new, "%s-%zd", file, v);

        if (rename(old, new) == 0) {
            fprintf(stderr, "*info* backing up %s, renamed -> %s\n", old, new);
        } else {
            //      perror(new);
        }
    }

    return 0;
}

// max value is inclusive
unsigned long getRandomLong(const unsigned long max) {
  const unsigned long l = getDevRandomLong();  // 0...ULONG_MAX
  unsigned long ret = max * (l * 1.0 / ULONG_MAX + 0.5);
  return ret;
}

// returns [0..max] inclusive
double getRandomDouble(const double max) {
  const unsigned long l = getDevRandomLong();
  double ret = max * (l * 1.0 / ULONG_MAX);
  return ret;
}

unsigned long getDevRandomLong(void) {
    FILE *fp = fopen("/dev/random", "rb");
    unsigned long c = 0;
    if (fread(&c, sizeof(unsigned long), 1, fp) == 0) {
        //      perror("random");
        c = time(NULL);
        fprintf(stderr, "*info* random seed from clock\n");
    }
    fclose(fp);
    if (c == 0) {
        fprintf(stderr, "*warning* the random number was 0. That doesn't happen often.\n");
    }
    return c;
}

unsigned char getDevRandom(void) {
    FILE *fp = fopen("/dev/random", "rb");
    unsigned char c = 0;
    if (fread(&c, sizeof(unsigned char), 1, fp) == 0) {
        //      perror("random");
        c = time(NULL);
        fprintf(stderr, "*info* random seed from clock\n");
    }
    fclose(fp);
    return c;
}


int entropyAvailable(void) {
    FILE *fp = fopen("/proc/sys/kernel/random/entropy_avail", "rt");
    int c = 0;
    if (fscanf(fp, "%d", &c) != 1) {
        fprintf(stderr, "*warning* couldn't get entropy_avail\n");
    }
    fclose(fp);
    return c;
}

unsigned long *randomGenerateLong(size_t len) {
    unsigned long *ret = malloc(len * sizeof(unsigned long));
    assert(len);
    for (size_t i = 0; i < len; i++) {
        ret[i] = getDevRandom();
    }
    return ret;
}

unsigned char *randomGenerate(size_t len) {
    unsigned char *ret = calloc(len, sizeof(unsigned char));
    assert(len);
    for (size_t i = 0; i < len; i++) {
        ret[i] = getDevRandom();
    }
    return ret;
}


unsigned char *passwordGenerateString(unsigned char *rnd, size_t len, const char *buf) {

    unsigned char *ret = malloc(len + 1);
    assert(ret);

    const int buflen = strlen(buf);

    for (size_t i = 0; i < len; i++) {
        unsigned char l = rnd[i];
        l = l % buflen;
        ret[i] = buf[l];
    }
    ret[len] = 0;

    return ret;
}


unsigned char *passwordGenerate(unsigned char *rnd, size_t len) {
    const char *buf = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    return passwordGenerateString(rnd, len, buf);
}


void readBenchmark(const int fd, const double seconds, const size_t blockSize, numListType *speed, numListType *latencyus) {
  //    numListType nl;
  //    nlInit(&nl, 1000000);
    //  const size_t blockSize = 2L*1024*1024;

    double start = timeAsDouble();
    size_t pos = 0;
    char *buffer = memalign(4096, blockSize);
    while ((timeAsDouble() < start + seconds) && keepRunning) {
        double time1 = timeAsDouble();
        int s = read(fd, buffer, blockSize);
        double time2 = timeAsDouble();
        if (s > 0) {
            double MBPS = TOMB(s) / (time2 - time1);
            if (MBPS > 0) {
	        if (speed) nlAdd(speed, MBPS);
                if (latencyus) nlAdd(latencyus, 1000000.0 * (time2 - time1));
            }
        } else {
            pos = 0;
            //      break;
        }
        pos += s;
    }
    double elapsed = timeAsDouble() - start;
    if (pos) {}

    if (!keepRunning) {
        fprintf(stderr, "*warning* interrupted\n");
    }
    printf("time:  \t%.3lf\n", elapsed);
    printf("size:  \t%zd\n", blockSize);
}


// returns the number of lines
int dumpFile(const char *fn, const char *regexstring, const int quiet) {

    regex_t regex;
    int reti;
    reti = regcomp(&regex, regexstring, REG_EXTENDED | REG_ICASE);
    if (reti) {
        fprintf(stderr, "*error* could not compile regex\n");
        return 0;
    }

    int linecount = 0;
    FILE *stream = fopen(fn, "rt");
    if (stream) {
        char *line = NULL;
        size_t len = 0;
        ssize_t nread;

        while ((nread = getline(&line, &len, stream)) != -1) {
            //      printf("Retrieved line of length %zu:\n", nread);

            reti = regexec(&regex, line, 0, NULL, 0);
            if (!reti) {
                linecount++;
                if (quiet == 0) {
                    fwrite(line, nread, 1, stdout);
                }
            }
        }

        free(line);
        fclose(stream);
    } else {
      if (quiet == 0) {
	perror(fn);
      }
    }

    regfree(&regex);
    return linecount;
}


void diskSpaceFromMount(const char *FSPATH, const char *s1, const char *total1, const char *free1, const char *per1) {
    struct statfs info;
    int ret = statfs(FSPATH, &info);

    if (ret == 0) {
        double total = info.f_bsize * info.f_blocks / 1024.0 / 1024 / 1024;
        double free = info.f_bsize * info.f_bavail / 1024.0 / 1024 / 1024;
        double per = (total - free) * 100.0 / total;

        printf("%-20s\t%10s\t%10s\t%s\n", s1, total1, free1, per1);
        printf("%-20s\t%10.2lf\t%10.2lf\t%.1lf%%\n", FSPATH, total, free, per);
    } else {
        perror(FSPATH);
    }
}


void loadEnvVars(char *filename) {
    FILE *fp = fopen(filename, "rt");
    if (!fp) {
        //silent
        //
    } else {
        //    printf("opened %s\n", filename);
        char *line = NULL;
        size_t len = 0;
        ssize_t nread;

        while ((nread = getline(&line, &len, fp)) != -1) {
            char *first = strtok(line, "=");
            char *second = strtok(NULL, "");
            if (second) {
	      if (isspace(second[strlen(second)-1])) {
                second[strlen(second) - 1] = 0;
	      }
	      setenv(first, second, 0);
		//		printf("*info* setting ENV %s\t->\t%s\n", first, second);
            }
        }
        free(line);
        fclose(fp);
    }
}

// new string
char *serialFromFD(int fd) {
    unsigned int major, minor;
    char *serial = NULL;

    if (majorAndMinor(fd, &major, &minor) == 0) {
        serial = getFieldFromUdev(major, minor, "E:ID_SERIAL=");
    }
    return serial;
}



#include <dirent.h>

size_t numberOfDirectories(char *dir) {
 struct dirent *dp;
 DIR *dfd;

 if ((dfd = opendir(dir)) == NULL) {
   perror(dir);
   return 0;
 }
 
 char filename_qfd[PATH_MAX] ;
 // char new_name_qfd[MAX_PATH] ;

 size_t count = 0;
 
 while ((dp = readdir(dfd)) != NULL) {
   struct stat stbuf ;

   char *end;
   strtod(dp->d_name, &end);

   if (end != NULL) {
     if (*end != 0) {
       continue;
     }
   }
     
   sprintf(filename_qfd, "%s/%s",dir,dp->d_name) ;
   if(stat(filename_qfd, &stbuf ) == -1 ) {
       perror(filename_qfd);
       //       printf("Unable to stat file: %s\n",filename_qfd) ;
       continue ;
  }

  if ( ( stbuf.st_mode & S_IFMT ) == S_IFDIR ) {
    count++;
    continue;
   // Skip directories
  } else {
    // not a dir
    //    char* new_name = get_new_name( dp->d_name ) ;// returns the new string
    // after removing reqd part
    //    sprintf(new_name_qfd,"%s/%s",dir,new_name) ;
    //    rename( filename_qfd , new_name_qfd ) ;
  }
 }
 return count;
}



#include <utmpx.h>

char *getcmdline(const size_t pid) {
  char s[PATH_MAX];
  sprintf(s, "/proc/%zd/cmdline", pid);
  int fp = open(s, O_RDONLY);
  if (fp > 0) {
    char buffer[1024];
    int rsz = read(fp, buffer, 1024);
    s[0] = 0;
    if (rsz > 0) {
      for (int i = 0; i < rsz; i++) {
	if (buffer[i] < 31) buffer[i] = 32;
      }
      buffer[rsz-1] = 0;
    }
    sprintf(s, "%s", buffer);
    close(fp);
    return strdup(s);
  } else {
    perror(s);
  }
  return NULL;
}

size_t who(const int quiet) 
{
    struct utmpx buffer;

    int tmpfile = open("/var/run/utmp", O_RDONLY);

    if (tmpfile < 0) {
      perror("/var/run/utmp");
      return -1;
    }

    if (quiet == 0) {
      printf("%-s\t %-s\t %-12s\t %-s\t %-15s\t%s\n", "USER", "TTY", "FROM", "PID", "WHAT", "WHEN");
    }

    size_t count = 0;
    while (read(tmpfile, &buffer, sizeof(struct utmpx)) != 0) {
      if (buffer.ut_type == USER_PROCESS) {
	if (quiet == 0) {
	  time_t tt = buffer.ut_tv.tv_sec;

	  struct tm tm = *localtime(&tt);
	  
	  char timestring[PATH_MAX];
	  strftime(timestring, 999, "%a %b %d %H:%M", &tm);

	  char *ttyn = ttyname(1);
	  int ttysame = 0;
	  if (ttyn && strlen(ttyn)>5) {
	    if (strncmp(ttyn+5, buffer.ut_line, __UT_LINESIZE)==0) {
	      ttysame = 1;
	    }
	  }

	  printf("%-s\t %-s\t %-12s\t %-d\t %-15s\t%s (%.0lf mins) %s\n", buffer.ut_user, buffer.ut_line, buffer.ut_host, buffer.ut_pid, getcmdline(buffer.ut_pid), timestring, ceil((time(NULL) -tt)/60.0), ((getppid()==buffer.ut_pid) || ttysame)?"*":"");
	}
	count++;
      }
    }
    
    return count;
}

size_t last(const int quiet) 
{
    struct utmpx buffer;

    int tmpfile = open("/var/log/wtmp", O_RDONLY);

    if (tmpfile < 0) {
      perror("/var/log/wtmp");
      return -1;
    }

    if (quiet == 0) {
      printf("%-10s\t %-8s\t %-12s\t %-s\t %-s\t%s\n", "USER", "TTY", "FROM", "PID", "WHAT", "WHEN");
    }

    size_t count = 0;
    time_t logintime = 0;
    int lastpid = -1;
    int needsnew = 0;

    char timestring[PATH_MAX];
    
    while (read(tmpfile, &buffer, sizeof(struct utmpx)) != 0) {
      //      if (buffer.ut_type == USER_PROCESS) {
	if (quiet == 0) {
	  time_t tt = buffer.ut_tv.tv_sec;

	  struct tm tm = *localtime(&tt);
	  


	  size_t logout = strlen(buffer.ut_user) == 0;
	  
	  if (buffer.ut_pid != lastpid) {
	    if (needsnew) {
	      printf("\n");
	    }
	    
	    strftime(timestring, 999, "%a %b %d %H:%M", &tm);
	    if (logout == 0) {
	      logintime = tt;
	      printf("%-10s\t %-8s\t %-15s\t %-8d\t%s", buffer.ut_user, buffer.ut_line, buffer.ut_host, buffer.ut_pid, timestring);
	      needsnew = 1;
	    } else {
	      needsnew = 0;
	    }
	  } else if (logout == 1) {
	    size_t duration = tt - logintime;
	    
	    strftime(timestring, 999, "%H:%M", &tm);
	    
	    printf(" - %s (%.0lf mins)\n", timestring, ceil(duration/60.0));
	    needsnew = 0;
	  }

	  lastpid = buffer.ut_pid;
	}
	count++;
      }
    //    }
    if (needsnew) {
      printf("\n");
    }
    
    return count;
 }





size_t listRunningProcessses() {
 struct dirent *dp;
 DIR *dfd;

 if ((dfd = opendir("/proc")) == NULL) {
   perror("/proc");
   return 0;
 }
 
 char filename_qfd[PATH_MAX] ;
 // char new_name_qfd[MAX_PATH] ;

 size_t count = 0;
 
 while ((dp = readdir(dfd)) != NULL) {
   struct stat stbuf ;

   char *end;
   strtod(dp->d_name, &end);

   if (end != NULL) {
     if (*end != 0) {
       continue;
     }
   }
     
   sprintf(filename_qfd, "/proc/%s", dp->d_name) ;
   if(stat(filename_qfd, &stbuf ) == -1 ) {
       perror(filename_qfd);
       //       printf("Unable to stat file: %s\n",filename_qfd) ;
       continue ;
  }

  if ( ( stbuf.st_mode & S_IFMT ) == S_IFDIR ) {

    char s[PATH_MAX];
    sprintf(s, "/proc/%s/status", dp->d_name);
    //    printf("%s\n", s);
    if (dumpFile(s, "(running|sleeping)", 1) > 0) {
      dumpFile(s, "(running|sleeping|name)", 0);
    }
    
    count++;
    continue;
   // Skip directories
  } else {
    // not a dir
    //    char* new_name = get_new_name( dp->d_name ) ;// returns the new string
    // after removing reqd part
    //    sprintf(new_name_qfd,"%s/%s",dir,new_name) ;
    //    rename( filename_qfd , new_name_qfd ) ;
  }
 }
 return count;
}


void printUptime() {
  size_t uptime = getUptime();

  printf("%zd time\n", uptime);
}


void timeToDHS(time_t t, time_t *d, time_t *h, time_t *s) {
  
  *d = t / (3600*24);
  *h = (t - (*d * 3600*24)) / 3600;
  *s = (t %3600)*60/3600;
}


// look at parent process, if cmdline contains sshd then the parent is sshd
size_t isSSHLogin() {
  int p_pid;

  p_pid = getppid(); /*parent process id*/
  int ret = 0;
  
  char *cmd = getcmdline(p_pid);
  if (cmd && strstr(cmd, "sshd")) {
    ret = 1;
  }
  free(cmd);
  
  return ret;
}

double getValueFromFile(const char *filename, const int quiet) {
  FILE *fp = fopen (filename, "rt");
  double value = NAN;
  if (fp) {
    char result[1024];
    int ret = fscanf(fp, "%s", result);
    if (ret == 1) {
      value = atof(result);
    }
    fclose(fp);
  } else {
    if (quiet == 0) { 
      perror(filename);
    }
  }
  return value;
}

  
void printBinary(const unsigned long value, const size_t bits) {
  for (int i = bits-1; i >= 0; i--) {
    if (value & (1L << i)) printf("1"); else printf("0");
    if ((i > 0) && ((i%8)==0)) printf (".");
  }
}


void logStringToFile(const char *fn, const char *string) {
  char *s = calloc(1024, 1);

  time_t t = time(NULL);
  struct tm tm = *localtime(&t);
  
  char timestring[100];
  strftime(timestring, 100, "%c %Z", &tm);


  sprintf(s, "%lf - %s - %s\n", timeAsDouble(), timestring, string);
  
  FILE *fp = fopen(fn, "at");
  if (fp) {

    fprintf(fp, "%s", s);
    fclose(fp);
  } else {
    perror(fn);
  }
  free(s);
}


// incoming 2 bytes, return 1
// incoming 4 bytes, return 2
// incoming 5 bytes, return 3 0x12345
unsigned char * hexString(const char *hexstring, size_t *retLen) {

  //  printf("input '%s'\n", hexstring);
  
  if (hexstring == NULL) {
    *retLen = 0;
    return NULL;
  }
  
  char *cov;
  size_t len = strlen(hexstring);

  // first string spaces
  char *strip = malloc(len+1);
  int p = 0;
  for (size_t i = 0; i < len; i++) {
    if (isalpha(hexstring[i]) || isdigit(hexstring[i])) {
      strip[p++] = hexstring[i];
    }
  }
  strip[p] = 0;
  len = strlen(strip);

  //  printf("strip: '%s'\n", strip);

  if ((len % 2) == 1) { // odd, prefix with 0
    cov = calloc(len + 2, 1);
    sprintf(cov, "0%s", strip);
  } else {
    cov = strdup(strip);
  }

  assert ((strlen(cov) % 2) == 0);
  *retLen = strlen(cov) / 2;
  unsigned char *val = calloc(*retLen, 1);


  char *pos = cov;
  for (size_t count = 0; count < *retLen; count++) {
    int ret = sscanf(pos, "%2hhx", &val[count]);
    if (ret != 1) {
      free(val); val = NULL; *retLen = 0;
      break;
    }
    pos += 2;
  }

  free(cov);
  free(strip);

  return val;
}


void fprintTimePrefix(FILE *fp) {
  time_t tt = time(NULL);
  
  struct tm tm = *localtime(&tt);
  
  char timestring[PATH_MAX];
  strftime(timestring, 999, "%c %Z", &tm);
  fprintf(fp, "[%s] ", timestring);
}


