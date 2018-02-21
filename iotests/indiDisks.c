#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <getopt.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <signal.h>
#include <ctype.h>
#include <syslog.h>

#include "utils.h"
#include "aioOperations.h"
#include "logSpeed.h"

int    keepRunning = 1;       // have we been interrupted
int    readySetGo = 0;
size_t blockSize = 64*1024; // default to 1MiB
double exitAfterSeconds = 0.7; // default timeout

typedef struct {
  int threadid;
  char *path;
  size_t total;
  logSpeedType logSpeed;
} threadInfoType;


void intHandler(int d) {
  (void)d;
  fprintf(stderr,"got signal\n");
  keepRunning = 0;
}


void process(int argc, char ** argv, int num) {
  //  fprintf(stderr,"%d %d\n", argc, num);
  for (size_t i = num; i < argc; i++) {
    if (!keepRunning) break;
    //    fprintf(stderr,"%zd : %s\n", i, argv[i]);
    size_t sz = blockDeviceSize(argv[i]);

    if (sz) {

      fprintf(stdout,"%s\t", argv[i]); fflush(stdout);

      double starttime = timedouble();
      long ret = writeNonBlocking(argv[i], blockSize, sz, exitAfterSeconds, 1);
      double elapsed = timedouble() - starttime;

      fprintf(stderr,"ret %ld\n", ret);
      fprintf(stdout, "%7.1lf", TOMiB(ret / elapsed)); fflush(stdout);
      sleep(1);
      starttime = timedouble();
      ret = readNonBlocking(argv[i], blockSize, sz, exitAfterSeconds, 1);
      elapsed = timedouble() - starttime;
      fprintf(stdout, "%7.1lf\n", TOMiB(ret / elapsed));
    } else {
      fprintf(stderr, "skipping: %s\n", argv[i]);
    }
  }
}


int handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "t:k:")) != -1) {
    switch (opt) {
    case 'k':
      blockSize = atoi(optarg) * 1024;
      if (blockSize < 1024) blockSize = 1024;
      break;
    case 't':
      exitAfterSeconds = atof(optarg);
      break;
    default:
      exit(-1);
    }
  }
  return optind;
}

int main(int argc, char *argv[]) {
  int num = handle_args(argc, argv);
  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  fprintf(stderr,"checking disks: blocksize=%zd (%zd KiB), timeout=%0.1lf (device, write MB/s, read MB/s)\n", blockSize, blockSize/1024, exitAfterSeconds);

  process(argc, argv, num);
  //  startThreads(argc, argv);
  return 0;
}
