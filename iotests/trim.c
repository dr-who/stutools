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

int keepRunning = 1;

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
