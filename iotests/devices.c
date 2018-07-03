#define _DEFAULT_SOURCE
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syslog.h>
#include <fcntl.h>
#include <errno.h>
#include <assert.h>

#include "devices.h"
#include "utils.h"

extern int keepRunning;

deviceDetails *addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs) {
  for (size_t i = 0; i < *numDevs; i++) {
    deviceDetails *dcmp = &(*devs)[i];
    //    fprintf(stderr,"looking at %s %p\n", dcmp->devicename, dcmp);
      if (strcmp(fn, dcmp->devicename)==0) {
	//      fprintf(stderr,"duplicate %s %p\n", fn, dcmp);
      return &(*devs)[i];
    }
  }
  //  fprintf(stderr,"didn't find %s, creating\n", fn);
  *devs = (deviceDetails*) realloc(*devs, (1024 * sizeof(deviceDetails))); // todo, max 1024 drives XXX
  (*devs)[*numDevs].fd = 0;
  (*devs)[*numDevs].bdSize = 0;
  (*devs)[*numDevs].devicename = strdup(fn);
  (*devs)[*numDevs].exclusive = 0;
  (*devs)[*numDevs].maxSizeGiB = 0;
  (*devs)[*numDevs].isBD = 0;
  
  //  if (verbose >= 2) {
  //    fprintf(stderr,"*info* adding -f '%s'\n", (*devs)[*numDevs].devicename);
  //  }

  deviceDetails *dcmp = &(*devs)[*numDevs];
  (*numDevs)++;
  return dcmp;
}

void freeDeviceDetails(deviceDetails *devs, size_t numDevs) {
  for (size_t f = 0; f < numDevs; f++) {
    if (devs[f].devicename) {
      free(devs[f].devicename);
    }
  }
  free(devs);
}

      
size_t loadDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs) {
  size_t add = 0;
  FILE *fp = fopen(fn, "rt");
  if (!fp) {
    perror(fn);
    return add;
  }

  char *line = NULL;
  size_t len = 0;
  ssize_t read;
  
  while ((read = getline(&line, &len, fp)) != -1) {
    //    printf("Retrieved line of length %zu :\n", read);
    line[strlen(line)-1] = 0; // erase the \n
    addDeviceDetails(line, devs, numDevs);
    //    addDeviceToAnalyse(line);
    add++;
    //    printf("%s", line);
  }
  
  free(line);

  fclose(fp);
  return add;
}

int startsWith(const char *pre, const char *str)
{
  size_t lenpre = strlen(pre), lenstr = strlen(str);
  return lenstr < lenpre ? 0 : strncmp(pre, str, lenpre) == 0;
}

int createFile(const char *filename, const double GiB) {
  int fd = 0;
  if (startsWith("/dev/", filename)) {
    fprintf(stderr,"*error* path error, will not create a file '%s' in the directory /dev/\n", filename);
    exit(-1);
  }
  fd = open(filename, O_RDWR | O_CREAT | O_TRUNC | O_DIRECT, S_IRUSR | S_IWUSR);
  if (fd < 0) {
    fprintf(stderr,"*info* creating the file with O_DIRECT didn't work...\n");
    fd = open(filename, O_RDWR | O_CREAT | O_TRUNC, S_IRUSR | S_IWUSR);
    if (fd < 0) {
      perror(filename);return 1;
    }
  }

  char *buf = NULL;
  CALLOC(buf, 1, 1024*1024);
  
  size_t towriteMiB = (size_t)(GiB * 1024) * 1024 * 1024;
  while (towriteMiB > 0 && keepRunning) {
    int towrite = MIN(towriteMiB, 1024*1024);
    if (write(fd, buf, towrite) != towrite) {
      perror("createFile");free(buf);return 1;
    }
    towriteMiB -= towrite;
  }
  fsync(fd);
  close(fd);
  free(buf);
  if (!keepRunning) {
    fprintf(stderr,"*warning* early size creation termination");
  }
  keepRunning = 1;
  return 0;
}

size_t numOpenDevices(deviceDetails *devs, size_t numDevs) {
  size_t count = 0;
  for (size_t i = 0; i < numDevs; i++) {
    if (devs[i].fd > 0) {
      count++;
    }
  }
  return count;
}

size_t openDevices(deviceDetails *devs, size_t numDevs, const size_t sendTrim, double maxSizeGB, size_t LOWBLKSIZE, size_t BLKSIZE, size_t alignment, int needToWrite, int dontUseExclusive) {
  size_t retBD = 0;
  
  //  int error = 0;
  
  char newpath[4096];
  //  CALLOC(newpath, 4096, sizeof(char));
    
  for (size_t i = 0; i < numDevs; i++) {
    memset(newpath, 0, 4096);
    // follow a symlink
    devs[i].fd = -1;
    char * ret = realpath(devs[i].devicename, newpath);
    if (!ret) {
      if (errno == ENOENT) {
	if (maxSizeGB == 0) {
	  fprintf(stderr,"*info* defaulting to 1 GiB size\n"); maxSizeGB = 1;
	}
	fprintf(stderr,"*info* no file with that name, creating '%s' with size %.2lf GiB...\n", devs[i].devicename, maxSizeGB*1.0);
	fflush(stderr);
	int rv = createFile(devs[i].devicename, maxSizeGB);
	if (rv != 0) {
	  fprintf(stderr,"*error* couldn't create file '%s'\n", devs[i].devicename);
	  exit(-1);
	}
	strcpy(newpath, devs[i].devicename);
      } else {
	perror(devs[i].devicename);
      }
    }

    size_t phy, log;
    if (isBlockDevice(newpath)) {
      /* -- From a Google search
	 The physical block size is the smallest size the device can
	 handle atomically. The smalles block it can handle without needing
	 to do a read-modify-write cycle. Applications should try to write
	 in multiples of that (and they do) but are not required to.

	 The logical block size on the other hand is the smallest size the
	 device can handle at all. Applications must write in multiples of
	 that.
      */
									
      char *suffix = getSuffix(newpath);
      char *sched = getScheduler(suffix);
    
      getPhyLogSizes(suffix, &phy, &log);
      //      if (verbose >= 2) {
      //	fprintf(stderr,"  *info* physical_block_size %zd, logical_block_size %zd\n", phy, log);
      //      }

      if (LOWBLKSIZE < log) {
	fprintf(stderr,"  *warning* the block size is lower than the device logical block size\n");
	LOWBLKSIZE = log;
	if (LOWBLKSIZE > BLKSIZE) BLKSIZE=LOWBLKSIZE;
      }
      
      // various warnings and informational comments
      if (LOWBLKSIZE < alignment) {
	LOWBLKSIZE = alignment;
	if (LOWBLKSIZE > BLKSIZE) BLKSIZE = LOWBLKSIZE;
	fprintf(stderr,"*warning* setting -k [%zd-%zd] because of the alignment of %zd bytes\n", LOWBLKSIZE/1024, BLKSIZE/1024, alignment);
      }


      free(sched); sched = NULL;
      free(suffix); suffix = NULL;
      
      if (needToWrite) { // if any writes
	if (dontUseExclusive < 3) { // specify at least -XXX to turn off exclusive
	  devs[i].fd = open(newpath, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
	} else {
	  fprintf(stderr,"  *warning* opening %s without O_EXCL\n", newpath);
	  devs[i].fd = open(newpath, O_RDWR | O_DIRECT | O_TRUNC);
	}
      } else { // only reads
	devs[i].fd = open(newpath, O_RDONLY | O_DIRECT); // if no writes, don't need write access
      }
      if (devs[i].fd < 0) {
	perror(newpath); goto cont;
      }

      devs[i].isBD = 1;
      devs[i].bdSize = blockDeviceSizeFromFD(devs[i].fd);
      // if too small then exit
      if (devs[i].bdSize <=1 ) {
	fprintf(stderr,"*fatal* block device is too small/missing.\n");
	exit(-1);
      }
      
      //      fprintf(stderr,"nnn %s\n", newpath);
      //      if (verbose >= 2) 
      //	fprintf(stderr,"*info* block device: '%s' size %zd bytes (%.2lf GiB)\n", newpath, devs[i].bdSize, TOGiB(devs[i].bdSize));
    } else {
      devs[i].fd = open(newpath, O_RDWR | O_EXCL); // if a file
      if (devs[i].fd < 0) {
	fprintf(stderr,"*info* opened file\n");
	devs[i].fd = open(newpath, O_RDWR | O_EXCL); // if a file
	if (devs[i].fd < 0) {
	  //	  fprintf(stderr,"maybe a file to create?\n");
	  perror(newpath); goto cont;
	}
	fprintf(stderr,"*warning* couldn't open in O_DIRECT mode (filesystem constraints)\n");
      }
      devs[i].isBD = 0;
      devs[i].bdSize = fileSize(devs[i].fd);

      //      fprintf(stderr,"*info* file: '%s' size %zd bytes (%.2lf GiB)\n", newpath, devs[i].bdSize, TOGiB(devs[i].bdSize));
    }
    
    if (i == 0) {
      retBD = devs[i].bdSize;
    } else {
      if (devs[i].bdSize < retBD) {
	retBD = devs[i].bdSize;
      }
    }
    
  cont: {}

    devs[i].exclusive = !dontUseExclusive;
    devs[i].maxSizeGiB = maxSizeGB;

    if (devs[i].fd <= 0) {
      fprintf(stderr,"*error* can't open device %s\n", devs[i].devicename);
    }
    //    assert(devs[i].fd > 0);
    //    assert(devs[i].fd < 256);

  } // i

  if (maxSizeGB > 0) {
    if (retBD > maxSizeGB*1024*1024*1024) {
      retBD = maxSizeGB*1024*1024*1024;
    }
  }

  return retBD;
}

void infoDevices(const deviceDetails *devList, const size_t devCount) {
  for (size_t f = 0; f < devCount; f++) {
    fprintf(stderr,"*info* [%zd], BD %d, '%s', fd %d, %zd bytes (%.2lf GiB), E %d, -G %.2g GiB\n", f, devList[f].isBD, devList[f].devicename, devList[f].fd, devList[f].bdSize, TOGiB(devList[f].bdSize), devList[f].exclusive, devList[f].maxSizeGiB);
  }
}

