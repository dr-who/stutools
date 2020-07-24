#define _XOPEN_SOURCE
#define _POSIX_C_SOURCE 200809L
#define _ISOC11_SOURCE
#define _DEFAULT_SOURCE

#include "jobType.h"
#include <signal.h>
#include <stdlib.h>
#include <malloc.h>
#include <assert.h>

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



void zapfunc(size_t pos, size_t deviceCount, int *selection, size_t blocksize, size_t writeblocksize, int print, char *block)
{
  if (blocksize) {}
  if (print) fprintf(stderr,"%9x (%5.3lf GiB):  ", (unsigned int)pos, TOGiB(pos));

  size_t mcount = 0;
  for (size_t i = 0; i < deviceCount; i++) {
    if (selection[i] > 0) {
      ssize_t ret = pwrite(selection[i], block, writeblocksize, pos);
      if (ret == (int)writeblocksize) {
        mcount++;
        if (print) fprintf(stderr,"%2d ", selection[i]);
      } else {
        //	perror("wow");
      }
    } else {
      if (print) fprintf(stderr,"   ");
    }
  }
  if (print) fprintf(stderr,"\t[%zd]\n", mcount);
}



void rotate(size_t pos, size_t deviceCount, int *selection, int *rotated, size_t blocksize, size_t writeblocksize, int print, char *block)
{
  if (writeblocksize) {}
  if (print) fprintf(stderr,"%9x (%5.1lf GiB):  ", (unsigned int)pos, TOGiB(pos));

  size_t mparity = 0;
  for (size_t i = 0; i < deviceCount; i++) {
    if (selection[i] > 0) {
      mparity++;
    }
  }
  //  fprintf(stderr,"Mparity %zd\n", mparity);

  // rotate
  int lastfd = -1, thisfd = -1, firstpos = -1;
  for (size_t i = 0; i < deviceCount; i++) {
    if (selection[i] > 0) {
      if (firstpos < 0) {
        firstpos = i;
      }
      thisfd = selection[i];
      if (lastfd > 0) {
        rotated[i] = lastfd;
      }
      lastfd = thisfd;
    }
  }
  rotated[firstpos] = lastfd;


  size_t mcount = 0, ok = 0;
  char *firstblock = aligned_alloc(4096, blocksize);
  for (size_t i = 0; i <deviceCount; i++) {
    if (selection[i] > 0) {
      mcount++;
      //            fprintf(stderr,"*info* read from %d, write to %d\n", selection[i], rotated[i]);

      ssize_t retr;

      if (mcount==1) {
        retr = pread(rotated[i], firstblock, blocksize, pos); // keep a copy of what it's clobbered
      }

      retr = pread(selection[i], block, blocksize, pos);

      ssize_t retw;
      if (mcount < mparity) {
        retw = pwrite(rotated[i], block, blocksize, pos);
      } else {
        retw = pwrite(rotated[i], firstblock, blocksize, pos);
      }


      if ((retw == (int)blocksize) && (retr == retw)) {
        ok++;
        if (print) fprintf(stderr,"%2d ", selection[i]);
      } else {
        perror("wow");
      }
    } else {
      if (print) fprintf(stderr,"   ");
    }
  }
  free(firstblock);
  if (print) fprintf(stderr,"\t[%zd]\n", ok);
}





int main(int argc, char *argv[])
{
  int opt;

  char *device = NULL;
  deviceDetails *deviceList = NULL;
  size_t deviceCount = 0;
  size_t kdevices = 0, mdevices = 0, blocksize = 256*1024, writeblocksize = 0;
  size_t startAt = 0*1024*1024, finishAt = 1024L*1024L*1024L*1;
  unsigned short seed = 0;
  int printevery = 1;
  int tripleX = 0;
  int zap = 1;
  char zapChar = 'Z';

  optind = 0;
  while ((opt = getopt(argc, argv, "I:k:m:G:g:R:b:B:qXzZ:r")) != -1) {
    switch (opt) {
    case 'b':
      blocksize = alignedNumber(atoi(optarg), 4096);
      break;
    case 'B':
      writeblocksize = alignedNumber(atoi(optarg), 4096);
      break;
    case 'k':
      kdevices = atoi(optarg);
      break;
    case 'G':
      finishAt = 1024L * 1024L * 1024L * atof(optarg);
      break;
    case 'g':
      startAt = 1024L * 1024L * 1024L * atof(optarg);
      break;
    case 'm':
      mdevices = atoi(optarg);
      break;
    case 'R':
      seed = atoi(optarg);
      break;
    case 'q':
      printevery *= 64;
      break;
    case 'z':
      zap = 1;
      break;
    case 'Z':
      zapChar = optarg[0];
      break;
    case 'r':
      zap = 0;
      break;
    case 'I':
    {}
    loadDeviceDetails(optarg, &deviceList, &deviceCount);
      //      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
    break;
    case 'X':
      tripleX++;
      break;
    default:
      exit(1);
      break;
    }
  }

  blocksize = alignedNumber(blocksize, 4096);
  if (blocksize == 0) blocksize = 4096;

  if (writeblocksize == 0) writeblocksize = blocksize;
  writeblocksize = alignedNumber(writeblocksize, 4096);

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
    fprintf(stderr,"*info* raidcheck -I devices.txt -k 10 -m 2 [options...)\n");
    fprintf(stderr,"\nOptions:\n");
    fprintf(stderr,"   -I file   specifies the list of underlying block devices\n");
    fprintf(stderr,"   -k n      the number of data devices\n");
    fprintf(stderr,"   -m n      the number of parity devices\n");
    fprintf(stderr,"   -g n      starting at n GiB (defaults byte 0)\n");
    fprintf(stderr,"   -G n      finishing at n GiB (defaults to 1 GiB)\n");
    fprintf(stderr,"   -b n      the block size to step through the devices\n");
    fprintf(stderr,"   -B n      the length of the block size to perturb (defaults to -b value)\n");
    fprintf(stderr,"   -I file   specifies the list of underlying block devices\n");
    fprintf(stderr,"   -XXX      opens the devices without O_EXCL. You will need this with RAID devices\n");
    fprintf(stderr,"\nUsage:\n");
    fprintf(stderr,"   raidcheck -I devices.txt -k 4 -m 2 -b 524288 -B 4096 -XXX\n");
    fprintf(stderr,"             Step through all devices in 512 KiB steps, setting the first 4096 bytes to 'Z'\n");
    fprintf(stderr,"             on at most 'm' devices at a time. The zapped blocks are shown asciily.\n\n");
    fprintf(stderr,"   raidcheck -I devices.txt -k 4 -m 2 -b 524288 -B 8192 -Z a -XXX\n");
    fprintf(stderr,"             Step through, setting the first 8192 bytes to 'a'\n\n");
    fprintf(stderr,"Bad md:\n");
    fprintf(stderr,"   # cat /dev/devices.txt\n");
    fprintf(stderr,"   /dev/ram0\n");
    fprintf(stderr,"   /dev/ram1\n");
    fprintf(stderr,"   /dev/ram2\n");
    fprintf(stderr,"   /dev/ram3\n");
    fprintf(stderr,"   # mdadm --create /dev/md0 --level=6 --raid-devices=4 $(cat devices.txt)\n");
    fprintf(stderr,"   # cat /sys/block/md0/md/mismatch_cnt \n");
    fprintf(stderr,"   82732\n");
    fprintf(stderr,"   # echo check > /sys/block/md0/md/sync_action\n");
    fprintf(stderr,"   # cat /sys/block/md0/md/mismatch_cnt \n");
    fprintf(stderr,"   0\n");
    fprintf(stderr,"   # raidcheck -I devices.txt -k 2 -m 2 -XXX\n");
    fprintf(stderr,"   # echo check > /sys/block/md0/md/sync_action\n");
    fprintf(stderr,"   # cat /sys/block/md0/md/mismatch_cnt\n");
    fprintf(stderr,"   28952\n");
    fprintf(stderr,"   # strings -n 4096 /dev/ram0 | grep ZZZ\n");
    fprintf(stderr,"\n   e.g. /dev/md0 is out of sync and it can't fix it.\n");

    exit(1);
  }

  fprintf(stderr,"*info* k = %zd, m = %zd, device count %zd, seed = %d\n", kdevices, mdevices, deviceCount, seed);
  if (kdevices + mdevices != deviceCount) {
    fprintf(stderr,"*error* k + m need to add to %zd\n", deviceCount);
    exit(1);
  }

  size_t maxSizeInBytes = 0;
  openDevices(deviceList, deviceCount, 0, &maxSizeInBytes, 4096, 4096, 4096, 1, tripleX, 1);
  infoDevices(deviceList, deviceCount);


  fprintf(stderr,"*info* number of open devices %zd\n", numOpenDevices(deviceList, deviceCount));
  if (numOpenDevices(deviceList, deviceCount) < deviceCount) {
    //not enough
    fprintf(stderr,"*error* not enough devices to open\n");
  } else {
    // enough drives
    if (mdevices < 2) {
      zap = 1; // if only 1 drive, you have to zap
    }

    if (zap) {
      fprintf(stderr,"*info* zap blocks\n");
    } else {
      fprintf(stderr,"*info* rotate blocks\n");
    }

    fprintf(stderr,"*info* raidcheck range [%.3lf GiB - %.3lf GiB) [%zd - %zd), block size = %zd, write block size = %zd, zapChar = '%c'\n", TOGiB(startAt), TOGiB(finishAt), startAt, finishAt, blocksize, writeblocksize, zapChar);
    srand48(seed);
    int *selection = malloc(deviceCount * sizeof(int));
    int *rotated = malloc(deviceCount * sizeof(int));

    char *block = aligned_alloc(4096, blocksize);
    memset(block, zapChar, blocksize);

    for (size_t pos = startAt,pr=0; pos < finishAt; pos += blocksize,pr++) {
      // pick k
      memset(selection, 0, deviceCount * sizeof(int));
      memset(rotated, 0, deviceCount * sizeof(int));
      for (size_t i = 0; i < mdevices; i++) {
        int r = lrand48() % deviceCount;
        while (selection[r] != 0) {
          r = lrand48() % deviceCount;
        }
        selection[r] = deviceList[r].fd;
      }

      if (zap) {
        zapfunc(pos, deviceCount, selection, blocksize, writeblocksize, (pr % printevery)==0, block);
      } else {
        rotate(pos, deviceCount, selection, rotated, blocksize, writeblocksize, (pr % printevery)==0, block);
      }
    }
    free(block);
    free(selection);
    free(rotated);

    for (size_t i = 0; i < deviceCount; i++) {
      if (deviceList[i].fd > 0) {
        close(deviceList[i].fd);
      }
    }
  }
  freeDeviceDetails(deviceList, deviceCount);

  fprintf(stderr,"*info* exiting.\n");
  fflush(stderr);

  exit(0);
}


