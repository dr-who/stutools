//#define _POSIX_C_SOURCE 200112L
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/uio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>

#include "utils.h"
#include "logSpeed.h"


int keepRunning = 0;
int device = 0;
size_t bdSize = 0;
size_t positions = 10;
int verify = 0;

size_t seed=31415;

int openDevice(const char *name) {
  int fd = open(name, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
  if (fd < 0) {
    perror("*error* can't open ");
    exit(-1);
  }
  bdSize = blockDeviceSizeFromFD(fd);
  if (bdSize < 1) {
    fprintf(stderr,"*error* block device is too small\n");exit(-1);
  }
  fprintf(stderr,"*info* size = %.1lf GB\n", TOGiB(bdSize));
  return fd;
}


int writeAndFlush(size_t blocksize, char *buffer, size_t bufferLen, size_t unique) {
  srand48(1);
  size_t maxBlock = (bdSize / blocksize) - 1;
  if (maxBlock <= 0) {
    fprintf(stderr,"*error* device is 0 bytes\n");exit(-1);
  }
  size_t count = 0;
  size_t globalCounter = 0;
  while (1) {
    size_t pos = lrand48() % (maxBlock + 1);
    off_t ret = lseek(device, pos * blocksize, SEEK_SET);
    if (ret == (off_t)-1) {
      fprintf(stderr,"*error* couldn't seek! to %zd\n", pos * blocksize);exit(-1);
    }

    sprintf(buffer,"%zd %zd\n", seed, globalCounter++);
    
    int ww = write(device, buffer, bufferLen);
    if (ww != bufferLen) {
      fprintf(stderr,"*error* write didn't complete!\n");exit(-1);
    }
    int ff = fsync(device);
    if (ff != 0) {
      fprintf(stderr,"*error* fsync didn't complete\n");exit(-1);
    }

    count++;
    fprintf(stderr,"write/fsync %zd (seed %zd) complete at position %zd (0x%zx), globalCount %zd\n", count, seed, pos * blocksize, pos * blocksize, globalCounter);
    
    
    if (count >= unique) {
      srand48(1);
      count = 0;
    }
  }
}
    


int readAndCheck(size_t blocksize, char *buffer, size_t bufferLen, size_t unique) {
  srand48(1);
  size_t maxBlock = (bdSize / blocksize) - 1;
  if (maxBlock <= 0) {
    fprintf(stderr,"*error* device is 0 bytes\n");exit(-1);
  }
  size_t count = 0;
  size_t globalCounter = 0;
  
  while (1) {
    size_t pos = lrand48() % (maxBlock + 1);
    off_t ret = lseek(device, pos * blocksize, SEEK_SET);
    if (ret == (off_t)-1) {
      fprintf(stderr,"*error* couldn't seek! to %zd\n", pos * blocksize);exit(-1);
    }

    //    sprintf(buffer,"31415 %zd\n", globalCounter++);
    
    int ww = read(device, buffer, bufferLen);
    if (ww != bufferLen) {
      fprintf(stderr,"*error* read didn't complete!\n");exit(-1);
    }

    size_t ss = 0, dd = 0;
    sscanf(buffer,"%zu %zu", &ss, &dd);
    if (seed == ss) {
      if (dd > globalCounter) {
	globalCounter = dd;
      }
    }
    //    fprintf(stderr,"--- %s %zd\n", ss, dd);

    count++;
    fprintf(stderr,"read %zd (seed %zd) complete, maximum globalCount %zd\n", count, seed, globalCounter);
    
    if (count >= unique) {
      srand48(1);
      count = 0;
    }
  }
}
    






void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "f:p:vR:")) != -1) {
    switch (opt) {
    case 'f':
      if (!device) {
	device = openDevice(optarg);
      }
      break;
    case 'R':
      seed = atoi(optarg);
      break;
    case 'p':
      positions = atoi(optarg);
      break;
    case 'v':
      verify = 1;
      break;
      
    default:
      break;
    }
  }
}


int main(int argc, char *argv[]) {
  handle_args(argc, argv);


  if (device) {
    char *randomBuffer = NULL;
    CALLOC(randomBuffer, 1, 65536);
    if (!verify) {
      writeAndFlush(65536, (char*)randomBuffer, 65536, positions);
    } else {
      readAndCheck(65536, (char*)randomBuffer, 65536, positions);
    }
  } else {
    fprintf(stderr,"./flushTester -f device -P 1000 -R seed\n");
    fprintf(stderr,"./flushTester -f device -P 1000 -R seed -v\n");
  }
    
  return 0;
}
