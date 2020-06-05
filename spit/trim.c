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


int main(int argc, char *argv[]) {

  if (argc < 2) {
    fprintf(stderr,"./trim /dev/device\n");
    exit(-1);
  }

  char *dev = argv[1];
  unsigned long low = 0;
  unsigned long high = fileSizeFromName(dev);

  if (isBlockDevice(dev) == 1) {
    int fd = open(dev, O_RDWR | O_EXCL | O_DIRECT );
    if (fd >= 0) {
      size_t d_max_bytes = 0, d_granularity = 0, d_zeroes = 0;
      getDiscardInfo(getSuffix(dev), &d_max_bytes, &d_granularity, &d_zeroes);
      
      if (fd >= 0) {
	performDiscard(fd, dev, low, high, d_max_bytes, d_granularity, 1);
	close(fd);
      } else {
	perror(dev);
      }
    } else {
      fprintf(stderr,"*error* couldn't open %s exclusively\n", dev);
    }
  } else {
    fprintf(stderr,"*error* a block device is required as an argument\n");
  }

  return 0;
}
