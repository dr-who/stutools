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
#include "positions.h"

int keepRunning = 1;
int verbose = 0;


int main(int argc, char *argv[])
{

  int opt, verbose = 0;
  size_t num = 100;
  char *device = NULL;
  size_t blockLow = 4096, blockHigh = 4096;
  size_t stepSize = 4096;
  char *posfile = NULL;
  size_t uniqSeedPerWrite = 0;
  size_t sequenceChoice = 1; // default to sequential
  size_t specifiedMaxBDSize = 0;

  while ((opt = getopt(argc, argv, "a:f:k:n:VP:us:G:" )) != -1) {
    switch (opt) {
    case 'a':
      stepSize = 1024 * atof(optarg);
      break;
    case 'f':
      device = strdup(optarg);
      break;
    case 'G':
      specifiedMaxBDSize = 1024L * 1024 * 1024L * atof(optarg);
      break;
    case 'k':
    {}
    char *endp = NULL;
    size_t bs = 1024 * strtod(optarg, &endp);
    if (bs < 512) bs = 512;
    blockLow = bs;
    blockHigh = bs;
    if (*endp == '-' || *endp == ':') {
      int nextv = atoi(endp+1);
      if (nextv > 0) {
        blockHigh = 1024 * nextv;
      }
    }
    break;
    case 'n':
      num = atoi(optarg);
      break;
    case 's':
      sequenceChoice = atoi(optarg);
      break;
    case 'P':
      posfile = strdup(optarg);
      break;
    case 'u':
      uniqSeedPerWrite = 1;
      break;
    case 'V':
      verbose++;
      break;
    }
  }

  if (!device) {
    fprintf(stderr,"*usage* -f device\n");
    exit(1);
  }

  size_t bdSize;
  if (isBlockDevice(device)==1) {
    bdSize = blockDeviceSize(device);
  } else {
    bdSize = fileSizeFromName(device);
  }
  if (specifiedMaxBDSize && (bdSize > specifiedMaxBDSize)) {
    fprintf(stderr,"*info* bdSize limited with -G. From %.3lf GiB  down to %.3lf GiB\n", TOGiB(bdSize), TOGiB(specifiedMaxBDSize));
    bdSize = specifiedMaxBDSize;
  }

  size_t startByte = 0;
  size_t finishByte = bdSize;
  size_t count = 0;
  size_t seed = 42;
  srand(42);

  positionContainer pc;
  positionContainerSetup(&pc, num);

  jobType job;
  jobInit(&job);
  jobAddBoth(&job, device, "w", -1);

  char *buffer;
  CALLOC(buffer, blockHigh, sizeof(char));



  if (!canOpenExclusively(device)) {
    perror("genpositions");
    exit(1);
  }

  int fd = open(device, O_RDWR | O_EXCL);
  if (fd < 0) {
    perror("genpositions");
    exit(1);
  }

  fprintf(stderr,"*info* bd size %zd (%.3lf GiB), block low %zd, block high %zd, step %zd, pos '%s'\n", bdSize, TOGiB(bdSize), blockLow, blockHigh, stepSize, posfile);
  for (size_t pos = startByte; pos < finishByte; ) {
    size_t len = randomBlockSize(blockLow, blockHigh, 12, lrand48());
    //    fprintf(stdout, "%zd\t%zd\n", pos, len);


    generateRandomBuffer(buffer, len, seed);
    checkRandomBuffer4k(buffer, len);
    //    buffer[len-1] = 0;
    //    fprintf(stderr,"%s\n", buffer);


    if (sequenceChoice == 0) {
      // align on 4096
      pc.positions[count].pos = (lrand48() % (bdSize >> 12)) << 12;
    } else {
      pc.positions[count].pos = pos;
    }
    pc.positions[count].len = len;
    pc.positions[count].action = 'W';
    pc.positions[count].submitTime = timedouble();
    pc.positions[count].seed = seed;
    pc.positions[count].deviceid = job.deviceid[0];

    size_t *posdest = (size_t*)buffer;
    size_t *uuiddest = (size_t*)buffer + 1;
    *posdest = pc.positions[count].pos;
    *uuiddest = pc.UUID;

    ssize_t ret = pwrite(fd, buffer, len, pc.positions[count].pos);
    if (ret != (ssize_t) len) {
      perror("genpositions");
      exit(1);
    }
    pc.positions[count].finishTime = timedouble();


    if (sequenceChoice == 1) {
      pos += stepSize;
      if (pos + blockHigh > bdSize) pos=0;
    }


    count++;
    if (uniqSeedPerWrite) seed++;

    if (count >= num)
      break;
  }
  close(fd);

  free(buffer);

  positionContainerHTML(&pc, "pileup.html");


  if (posfile) {
    positionContainerSave(&pc, stdout, bdSize, 0, &job);
  }
  positionContainerFree(&pc);

  free(device);
  free(posfile);

  return 0;
}
