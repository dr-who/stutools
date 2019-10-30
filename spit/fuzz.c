#define _POSIX_C_SOURCE 200809L
#define _ISOC11_SOURCE

#include "jobType.h"
#include <signal.h>
#include <stdlib.h>

/**
 * spit.c
 *
 * the main() for ./fuzz, a RAID stripe aware fuzzer
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"
#include "diskStats.h"

#define DEFAULTTIME 10
  
int verbose = 0;
int keepRunning = 1;


int main(int argc, char *argv[]) {
  int opt;

  char *device = NULL;
  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0;
  
  optind = 0;
  while ((opt = getopt(argc, argv, "I:")) != -1) {
    switch (opt) {
    case 'I':
      {}
      size_t added = loadDeviceDetails(optarg, &deviceList, &deviceCount);
      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
      break;
    default:
      exit(1);
      break;
    }
  }

  // first assign the device
  // if one -f specified, add to all jobs
  // if -F specified (with n drives), have c x n jobs
  if (deviceCount) {
    // if we have a list of files, overwrite the first one, ignoring -f
    if (device) {
      fprintf(stderr,"*warning* ignoring the value from -f\n");
    }
    device = deviceList[0].devicename;
  } else {
    fprintf(stderr,"*error* need -I devices.txt\n");
    exit(1);
  }

  size_t maxSizeInBytes = 0;
  openDevices(deviceList, deviceCount, 0, &maxSizeInBytes, 4096, 4096, 4096, 1, 1, 16, 1);
  infoDevices(deviceList, deviceCount);

  fprintf(stderr,"*info* num open %zd\n", numOpenDevices(deviceList, deviceCount));

  for (size_t i = 0; i < deviceCount; i++) {
    unsigned char *mem = aligned_alloc(4096, 4096);
    lseek(deviceList[i].fd, 0*1024*1024, SEEK_SET);
    ssize_t ret = read(deviceList[i].fd, mem, 4096);
    //    fprintf(stderr,"ret %zd, fd = %d\n", ret, deviceList[i].fd);
    if (ret == 4096) {
      for (size_t j = 0; j < 120; j++) {
	unsigned char c = (unsigned char) mem[j];
	if (c<32) c='_';
	fprintf(stderr,"%c", c);
      }
      fprintf(stderr,"\n");
    } else {
      perror("wow");
    }
  }

  
  fprintf(stderr,"*info* exiting.\n"); fflush(stderr);
  
  exit(0);
}
  
  
