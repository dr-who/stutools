#ifdef __linux__
#define _GNU_SOURCE
#endif

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <time.h>
#include <assert.h>

#include "utilstime.h"
#include "utils.h"

int keepRunning = 1;

void *readData(const int fd, const size_t sz) {
  void *d = NULL;
  int ret = posix_memalign(&d, 4096, sz);
  if (ret == 0) {
    size_t pos = 0;
    while ((ret = pread(fd, d, sz - pos, pos)) > 0) {
      pos += ret;
    }
    fprintf(stderr,"*info* read %zd bytes from fd %d\n", sz, fd);
  } else {
    fprintf(stderr,"can't allocate!\n");
    exit(1);
  }
  return d;
}

void checkData(const int fd, const void *p, const size_t sz) {
  void *check = NULL;
  int ret = posix_memalign(&check, 4096, sz);
  if (ret == 0) {
    size_t pos = 0;
    while ((ret = pread(fd, check, sz - pos, pos)) > 0) {
      pos += ret;
    }
    assert(pos == sz);
    fprintf(stderr,"*info* checking %zd bytes from fd %d\n", sz, fd);
    int ch = memcmp(p, check, sz);
    if (ch == 0) {
      fprintf(stderr, "*info* device contents as initially read\n");
    } else {
      fprintf(stderr, "*info* check FAILED. oh my!\n");
      exit(1);
    }
  } else {
    fprintf(stderr,"can't allocate!\n");
    exit(1);
  }
  free(check);
}

size_t iopos = 0;

void benchmark(const int fd, const void *p, const size_t sz, const size_t blocksz, const double testtime, const int readtest, const int seq, FILE *log, const double yoff) {
  (void)p;
  void *readmem = NULL;
  ssize_t ret = 0;
  
  if (readtest) {
    ret = posix_memalign(&readmem, 4096, sz);
    if (ret != 0) {
      exit(1);
    }
  }
  double start = timeAsDouble(), end;
  srand(time(NULL));

  size_t maxblocks = sz / blocksz;
  size_t ios = 0;

  size_t testblock = 0;

  numListType nl;
  nlInit(&nl, 100000);

  while (((end = timeAsDouble()) - start) <= testtime || ios < 40) {
    if (seq) {
      testblock++;
      if (testblock >= maxblocks) testblock = 0; // wrap
    } else {
      testblock = rand()%maxblocks;
    }
    assert (testblock < maxblocks);
    if (readtest) {
      ret = pread(fd, (char*)readmem + (testblock*blocksz), blocksz, testblock * blocksz);
    } else {
      //     fprintf(stderr,"-->offset %zd, max %zd\n", off, maxblocks);
      ret = pwrite(fd, (char*)p + (testblock*blocksz), blocksz, testblock*blocksz);
      if (ret < 0) {
	perror("write");
	exit(1);
      }
      if (ret != (ssize_t)blocksz) {
	fprintf(stderr,"*short write*\n");
      }
      if (ret < 0) {
	perror("write");
      }
      //      assert(off - startoff == blocksz);
    }
    const double fin = timeAsDouble() - end;
    iopos += blocksz;
    nlAdd(&nl, fin *1000);

    fprintf(log, "%lf %zd\n", timeAsDouble(), iopos);
    
    if (ret < 0) {
      perror(readtest ? "READ" : "WRITE");
      exit(1);
    }
    ios++;
    //    fprintf(stderr, "%zd\n", testblock);
  }
  double iops = ios / (end - start);
  fprintf(stderr,"*info* %s %s blocksize=%7zd: %6zd IOs in %6.3lf seconds (99%% <= %6.1lf ms, s.d. %5.1lf ms), is %6.0lf IO/s, %4.0lf MB/s\n",
	  seq ? "Seq" : "Rnd", readtest ? "READ " : "WRITE", blocksz, ios, end - start, nlSortedPos(&nl, 0.99), nlSD(&nl), iops, iops * blocksz/1000/1000);
  nlFree(&nl);

  if (readtest) {
    free(readmem);
  }
}
      
      
      
      
    
  


int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;

  char *device = NULL;
  if (argc > 1) {
    device = argv[1];
  }
  
  double testtime = 0.5;
  if (argc > 2) {
    testtime = atof(argv[2]);
    if (testtime < 0.001) {
      testtime = 0.5;
    }
  }
  if (device == NULL) {
    printf("usage: testdevice <blockdevice>\n");
    exit(0);
  }

  char *suffix = getSuffix(device);

  char s[100];
  sprintf(s, "/sys/block/%s/diskseq", suffix);
  double yoff = getValueFromFile(s, 1);
  if (argc > 3) {
    yoff = atof(argv[3]);
  }




  size_t sz = 1024 * 1024 * 100;


  int fd = open(device, O_RDWR | O_EXCL | O_DIRECT);
  if (fd >= 0) {
    char *v = NULL, *m = NULL;
    fprintf(stderr,"*info* '%s': %s %s, %.0lf GB (yoff %.1lf)\n", device, v=vendorFromFD(fd), m=modelFromFD(fd), blockDeviceSizeFromFD(fd)/1000.0/1000/1000, yoff);
    
    sprintf(s, "%s-%s-%.0lf", v, m, yoff);
    FILE *gnuplot = fopen(s, "wt");
    if (gnuplot == NULL) {
      perror(s);
      exit(EXIT_FAILURE);
    }
  

    
    free(v); free(m);
    fprintf(stderr,"*info* allocating fd %d to %s\n", fd, device);
    void *p = readData(fd, sz);

    for (int seq = 0; seq <= 1; seq++) {
      for (int rw = 0; rw <= 1; rw++) {
	benchmark(fd, p, sz, 4096, testtime, rw, seq, gnuplot, yoff);
	benchmark(fd, p, sz, 65536, testtime, rw, seq, gnuplot, yoff);
	benchmark(fd, p, sz, 1024*1024, testtime, rw, seq, gnuplot, yoff);
      }
    }

    fclose(gnuplot);
    checkData(fd, p, sz);
    close(fd);
    free(p);
    fprintf(stderr,"*info* test passed.\n");
  } else {
    perror(device);
    exit(EXIT_FAILURE);
  }


  return 0;
}

