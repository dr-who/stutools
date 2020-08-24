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



void xorfunc(size_t pos, size_t deviceCount, int *selection, size_t blocksize, size_t writeblocksize, int print, char *block)
{
  if (blocksize) {}
  if (print) fprintf(stderr,"%9x (%5.3lf GiB):  ", (unsigned int)pos, TOGiB(pos));

  size_t mcount = 0;
  for (size_t i = 0; i < deviceCount; i++) {
    if (selection[i] > 0) {
      ssize_t ret = pread(selection[i], block, writeblocksize, pos);
      for (ssize_t j = 0; j < ret; j++) {
	block[j] = block[j] ^ 127;
      }
      ret = pwrite(selection[i], block, writeblocksize, pos);
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




void decimalfunc(size_t pos, size_t deviceCount, int *selection, size_t blocksize, size_t writeblocksize, int print, char *block)
{
  fprintf(stderr,"*info* decimalfunc\n");
  if (blocksize) {}
  if (print) fprintf(stderr,"%9x (%5.3lf GiB):  ", (unsigned int)pos, TOGiB(pos));

  size_t mcount = 0;
  for (size_t i = 0; i < deviceCount; i++) {
    if (selection[i] > 0) {
      ssize_t ret = pread(selection[i], block, writeblocksize, pos);
      for (ssize_t j = 0; j < ret; j++) {
	if ((block[j] >= '0') && (block[j] <= '9')) {
	  block[j] = '0' + ('9' - (int)block[j]);
	}
      }
      ret = pwrite(selection[i], block, writeblocksize, pos);
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



void rotatefunc(size_t pos, size_t deviceCount, int *selection, int *rotated, size_t blocksize, size_t writeblocksize, int print, char *block)
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

  for (size_t i = 0; i < deviceCount; i++) {
    //    fprintf(stderr,"%d   %d\n", selection[i], rotated[i]);
  }


  size_t mcount = 0, ok = 0;
  char *firstblock = aligned_alloc(4096, blocksize);
  for (size_t i = 0; i <deviceCount; i++) {
    if (selection[i] > 0) {
      mcount++;
      ssize_t retr = 0, retw = 0;

      if (mcount==1) {
	//	fprintf(stderr,"read FIRST from %d\n", rotated[i]);
        retr = pread(rotated[i], firstblock, blocksize, pos); // keep a copy of what it's clobbered
	if (retr < 0) perror("raidcorrupt");
      }

      //      fprintf(stderr,"mcount %zd par %zd\n", mcount, mparity);
      if (mcount < mparity) {
	retr = pread(selection[i], block, blocksize, pos);
	if (retr < 0) perror("raidcorrupt");
	//	fprintf(stderr,"read from %d\n", selection[i]);
        retw = pwrite(rotated[i], block, blocksize, pos); // copy the most recent read to the rotated
	if (retw != (ssize_t) blocksize) perror("raidcorrupt");
	//	fprintf(stderr,"write to %d\n", rotated[i]);
      } else {
        retw = pwrite(rotated[i], firstblock, blocksize, pos); // finish off
	if (retw != (ssize_t) blocksize) perror("raidcorrupt");
	//	fprintf(stderr,"write first to %d\n", rotated[i]);
      }
      if (print) fprintf(stderr,"%3d", rotated[i]);
      ok++;
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
  int xor = 0;
  int rotate = 0;
  int decimal = 0;
  char zapChar = 'Z';

  optind = 0;
  while ((opt = getopt(argc, argv, "I:k:m:G:g:R:b:B:qxXzZ:rf:d")) != -1) {
    switch (opt) {
    case 'b':
      blocksize = alignedNumber(atoi(optarg), 4096);
      break;
    case 'B':
      writeblocksize = alignedNumber(atoi(optarg), 4096);
      break;
    case 'd':
      decimal = 1;
      xor = 0;
      rotate = 0;
      zap = 0;
      break;
    case 'k':
      kdevices = atoi(optarg);
      break;
    case 'G':
      if (strchr(optarg,'M') || strchr(optarg,'m')) {
	finishAt = (size_t)(1024.0 * 1024.0 * atof(optarg));
      } else {
	finishAt = (size_t)(1024.0 * 1024.0 * 1024.0 * atof(optarg));
      }
      //      fprintf(stderr,"*info* finish at %zd (%.4lf GiB, %.3lf MiB)\n", finishAt, TOGiB(finishAt), TOMiB(finishAt));
      break;
    case 'g':
      if (strchr(optarg,'M') || strchr(optarg,'m')) {
	startAt = (size_t)(1024.0 * 1024.0 * atof(optarg));
      } else {
	startAt = (size_t)(1024.0 * 1024.0 * 1024.0 * atof(optarg));
      }
      //      fprintf(stderr,"*info* start at %zd (%.4lf GiB, %.3lf MiB)\n", startAt, TOGiB(startAt), TOMiB(startAt));
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
      xor = 0;
      rotate = 0;
      decimal = 0;
      break;
    case 'Z':
      zapChar = optarg[0];
      break;
    case 'r':
      rotate = 1;
      zap = 0;
      xor = 0;
      decimal = 0;
      break;
    case 'f':
      addDeviceDetails(strdup(optarg), &deviceList, &deviceCount);
      break;
    case 'I':
    {}
    loadDeviceDetails(optarg, &deviceList, &deviceCount);
      //      fprintf(stderr,"*info* added %zd devices from file '%s'\n", added, optarg);
    break;
    case 'x':
      xor = 1;
      zap = 0;
      rotate = 0;
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
  if (writeblocksize > blocksize) writeblocksize = blocksize;

  writeblocksize = alignedNumber(writeblocksize, 4096);

  // align the numbers to the blocksize
  startAt = alignedNumber(startAt, blocksize);
  finishAt = alignedNumber(finishAt, blocksize);
  
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
    fprintf(stderr,"*info* raidcorrupt -I devices.txt -k 10 -m 2 [options...)\n");
    fprintf(stderr,"\nCorrupt underlying devices under a software RAID array\n");
    fprintf(stderr,"\nOptions:\n");
    fprintf(stderr,"   -I file   specifies the list of underlying block devices\n");
    fprintf(stderr,"   -k n      the number of data devices\n");
    fprintf(stderr,"   -m n      the number of parity devices\n");
    fprintf(stderr,"   -z        zap the block, make all bytes 'Z'\n");
    fprintf(stderr,"   -x        XOR the block with 0x7f. Run twice to revert.\n");
    fprintf(stderr,"   -r        rotate the blocks between devices.\n");
    fprintf(stderr,"   -d        change digits [0-9] to 9-value. Run twice to revert.\n");
    fprintf(stderr,"   -g n      starting at n GiB (defaults byte 0)\n");
    fprintf(stderr,"   -g 16M    starting at 16 MiB\n");
    fprintf(stderr,"   -G n      finishing at n GiB (defaults to 1 GiB)\n");
    fprintf(stderr,"   -G 32M    finishing at 32 MiB\n");
    fprintf(stderr,"   -b n      the block size to step through the devices (defaults to 256 KiB)\n");
    fprintf(stderr,"   -B n      the length of the block size to perturb (defaults to -b value)\n");
    fprintf(stderr,"   -R seed   sets the starting seed\n");
    fprintf(stderr,"   -XXX      opens the devices without O_EXCL. You will need this with RAID devices\n");
    fprintf(stderr,"   -q        quieter mode. Output 1/64 lines each 'q'. -qq is very quiet\n");
    fprintf(stderr,"   -f dev    the unusual case of specifying a single block device\n");
    fprintf(stderr,"\nUsage:\n");
    fprintf(stderr,"   Assuming the RAID array has been created with 4+2 (4 data, plus 2 parity)\n\n");
    fprintf(stderr,"   raidcorrupt -k 6 -m 0    # steps of 256 KiB, don't change any data\n");
    fprintf(stderr,"   raidcorrupt -k 5 -m 1    # steps of 256 KiB, corrupt a device every 256KiB\n");
    fprintf(stderr,"   raidcorrupt -k 4 -m 2    # pick two devices for every 256KiB stripe and corrupt\n");
    fprintf(stderr,"   raidcorrupt -k 3 -m 3    # pick three devices every 256KiB. Causes data loss\n");
    fprintf(stderr,"   raidcorrupt -I devices.txt -k 4 -m 2 -g 16M -XXX\n\n");
    fprintf(stderr,"   raidcorrupt -I devices.txt -k 4 -m 2 -b 524288 -B 4096 -XXX\n");
    fprintf(stderr,"             Step through all devices in 512 KiB steps, setting the first 4096 bytes to 'Z'\n");
    fprintf(stderr,"             on at most 'm' devices at a time. The zapped blocks are shown asciily.\n\n");
    fprintf(stderr,"   raidcorrupt -I devices.txt -k 4 -m 2 -b 524288 -B 8192 -Z a -XXX\n");
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
    fprintf(stderr,"   # raidcorrupt -I devices.txt -k 2 -m 2 -XXX\n");
    fprintf(stderr,"   # echo check > /sys/block/md0/md/sync_action\n");
    fprintf(stderr,"   # cat /sys/block/md0/md/mismatch_cnt\n");
    fprintf(stderr,"   28952\n");
    fprintf(stderr,"   # strings -n 4096 /dev/ram0 | grep ZZZ\n");
    fprintf(stderr,"\n   e.g. /dev/md0 is out of sync and it can't fix it.\n");

    exit(1);
  }

  fprintf(stderr,"*info* aligned start:  %lx (%zd, %.3lf MiB, %.4lf GiB)\n", startAt, startAt, TOMiB(startAt), TOGiB(startAt));
  fprintf(stderr,"*info* aligned finish: %lx (%zd, %.3lf MiB, %.4lf GiB)\n", finishAt, finishAt, TOMiB(finishAt), TOGiB(finishAt));

  fprintf(stderr,"*info* k = %zd, m = %zd, device count %zd, seed = %d\n", kdevices, mdevices, deviceCount, seed);
  if (kdevices + mdevices != deviceCount) {
    fprintf(stderr,"*error* k + m need to sum to %zd\n", deviceCount);
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
    if (rotate && mdevices < 2) {
      fprintf(stderr,"*warning* rotation doesn't work with %zd parity\n", mdevices);
    }

    if (zap) {
      fprintf(stderr,"*info* zap blocks\n");
    } else if (xor) {
      fprintf(stderr,"*info* read/XOR/write blocks\n");
    } else { 
      fprintf(stderr,"*info* rotate blocks\n");
    }

    fprintf(stderr,"*info* range [%.3lf GiB - %.3lf GiB) [%zd - %zd), block size = %zd, write block size = %zd\n", TOGiB(startAt), TOGiB(finishAt), startAt, finishAt, blocksize, writeblocksize);
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
      } else if (decimal) {
        decimalfunc(pos, deviceCount, selection, blocksize, writeblocksize, (pr % printevery)==0, block);
      } else if (xor) {
        xorfunc(pos, deviceCount, selection, blocksize, writeblocksize, (pr % printevery)==0, block);
      } else {
        rotatefunc(pos, deviceCount, selection, rotated, blocksize, writeblocksize, (pr % printevery)==0, block);
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


