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
#include <linux/fs.h>
#include <sys/ioctl.h>
#include "utils.h"
#include "logSpeed.h"

int keepRunning = 1;

int trimDevice(int fd, char *path, unsigned long low, unsigned long high) {
  unsigned long range[2];
  
  range[0] = low;
  range[1] = high;

  fprintf(stderr,"*info* sending trim/BLKDISCARD command to %s [%ld, %ld] [%.3lf GiB, %.3lf GiB]\n", path, range[0], range[1], TOGiB(range[0]), TOGiB(range[1]));
  
  int err = 0;
  if ((err = ioctl(fd, BLKDISCARD, &range))) {
    fprintf(stderr, "*error* %s: BLKDISCARD ioctl failed, error = %d\n", path, err);
  }
  
  fdatasync(fd);
  fsync(fd);

  return err;
}


int main(int argc, char *argv[]) {

  if (argc < 4) {
    fprintf(stderr,"./trim /dev/device lowbytepos highbytepos\n");
    exit(-1);
  }

  char *dev = argv[1];
  unsigned long low = atol(argv[2]);
  unsigned long high = atol(argv[3]);

  if (((low / 512) * 512) != low) {
    fprintf(stderr,"*error* low isn't a multiple of 512\n");
    exit(-2);
  }
  
  if (((high / 512) * 512) != high) {
    fprintf(stderr,"*error* high isn't a multiple of 512\n");
    exit(-2);
  }
  
int fd = open(dev, O_RDWR | O_EXCL | O_DIRECT );
  
  if (fd >= 0) {
    size_t bdsize = blockDeviceSizeFromFD(fd);
    if (high > bdsize) {
      fprintf(stderr,"*warning* high value of %lu greated than size of device %zd\n", high, bdsize);
      high = bdsize;
    }
    
    trimDevice(fd, dev, low, high); 
    close(fd);
  } else {
    perror(dev);
  }

  return 0;
}
