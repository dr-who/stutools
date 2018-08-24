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

  fprintf(stderr,"*info* sending trim command to %s [%ld, %ld] [%.1lf GiB, %.1lf GiB]\n", path, range[0], range[1], TOGiB(range[0]), TOGiB(range[1]));
  
  int err = 0;
  if ((err = ioctl(fd, BLKDISCARD, &range))) {
    fprintf(stderr, "*error* %s: BLKDISCARD ioctl failed (maybe read only mode only?)\n", path);
  }

  fdatasync(fd);
  fsync(fd);

  return err;
}


int main(int argc, char *argv[]) {
  //  int index = handle_args(argc, argv);
  int index = 1;
  //  signal(SIGTERM, intHandler);
  //  signal(SIGINT, intHandler);

  for (size_t i = index; i < argc; i++) {
    int fd = open(argv[i], O_RDWR | O_EXCL | O_DIRECT );
    if (fd >= 0) {
      size_t bdsize = blockDeviceSizeFromFD(fd);
      trimDevice(fd, argv[i], 0, bdsize - 512); 
      close(fd);
    } else {
      perror("open");
    }
  }

  return 0;
}
