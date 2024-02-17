#ifdef __linux__
#define _GNU_SOURCE
#endif

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <time.h>
#include <getopt.h>
#include <assert.h>

#include "blockdevices.h"
#include "utilstime.h"
#include "utils.h"

int keepRunning = 1;

void *readData(const int fd, const size_t sz) {
  void *d = NULL;
  int ret = posix_memalign(&d, 4096, sz);
  if (ret == 0) {
    fprintf(stderr,"*info* reading %zd bytes from fd %d\n", sz, fd);
    size_t pos = 0;
    while ((ret = pread(fd, d, sz - pos, pos)) > 0) {
      pos += ret;
    }
  } else {
    fprintf(stderr,"can't allocate!\n");
    exit(1);
  }
  return d;
}

// return 0 is ok
int clobberData(const int fd, size_t pos, const size_t sz, const char *buf) {
  
  int ret;
  size_t off = 0;
  while ((ret = pwrite(fd, buf, sz -off, pos + off)) > 0) {
    //    fprintf(stderr,"*info* clobbering pos=%zd len = %zd, ret = %d\n", pos, sz, ret);
    off += ret;
  }
  if (ret < 0) {
    //    perror("clobber");
    return -1;
  } else {
    //    fprintf(stderr,"written\n");
    return 0;
  }
}

int checkDidWrite(const int fd, char *buf, size_t sz, size_t pos) {
  char *check = NULL;
  int ret = posix_memalign((void*)&check, 4096, sz);
  bzero(check, 4096);
  
  if (ret == 0) {
    fprintf(stderr,"*info* checking %zd bytes from offset %zd\n", sz, pos);
    ret = pread(fd, check, 4096, pos);
    /*      fprintf(stderr,"read %d\n", ret);
      abort();
      pos += ret;
      }*/
    //    assert(ret == (int)sz);
    int ch = 0;
    for (size_t i =0; i < 10; i++) {
      fprintf(stderr,"'%c' '%c'\n", buf[i], check[i]);
      if (buf[i] != check[i]) ch= 1;
    }
    if (ch == 0) {
      fprintf(stderr, "*info* device retrieved the last write\n");
      ret = 0;
    } else {
      fprintf(stderr, "*info* device said yes BUT CAN'T RESTORE IT!\n");
      ret = 1;
    }
  } else {
    fprintf(stderr,"can't allocate\n");
    exit(1);
  }
  free(check);
  return ret;
}


void checkData(const int fd, const void *p, const size_t sz) {
  void *check = NULL;
  int ret = posix_memalign(&check, 4096, sz);
  if (ret == 0) {
    size_t pos = 0;
    fprintf(stderr,"*info* checking %zd bytes%d\n", sz, fd);
    while ((ret = pread(fd, check, sz - pos, pos)) > 0) {
      pos += ret;
    }
    assert(pos == sz);
    int ch = memcmp(p, check, sz);
    if (ch == 0) {
      fprintf(stderr, "*info* device contents as initially read\n");
    } else {
      fprintf(stderr, "*info* check FAILED. oh my!\n");
      exit(1);
    }
  } else {
    fprintf(stderr,"can't allocate!\n");
    exit(1);
  }
  free(check);
}

void do_benchmark(const int fd, const void *p, const size_t sz, const size_t blocksz, const double testtime, const int readtest, const int seq, FILE *log, size_t *iopos, const double starttime, const size_t run) {
  (void)p;
  void *readmem = NULL;
  ssize_t ret = 0;
  
  if (readtest) {
    ret = posix_memalign(&readmem, 4096, sz);
    if (ret != 0) {
      exit(1);
    }
  }
  double start = timeAsDouble(), end;

  size_t maxblocks = sz / blocksz;
  size_t ios = 0;

  size_t testblock = 0;

  numListType nl;
  nlInit(&nl, 100000);

  while (((end = timeAsDouble()) - start) <= testtime || ios < 40) {
    if (end - start > 3) break;
    if (seq) {
      testblock++;
      if (testblock >= maxblocks) testblock = 0; // wrap
    } else {
      testblock = rand()%maxblocks;
    }
    assert (testblock < maxblocks);
    if (readtest) {
      ret = pread(fd, (char*)readmem + (testblock*blocksz), blocksz, testblock * blocksz);
    } else {
      //     fprintf(stderr,"-->offset %zd, max %zd\n", off, maxblocks);
      ret = pwrite(fd, (char*)p + (testblock*blocksz), blocksz, testblock*blocksz);
      if (ret < 0) {
	perror("write");
	exit(1);
      }
      if (ret != (ssize_t)blocksz) {
	fprintf(stderr,"*short write*\n");
      }
      if (ret < 0) {
	perror("write");
      }
      //      assert(off - startoff == blocksz);
    }
    const double fin = timeAsDouble() - end;
    (*iopos) += blocksz;
    nlAdd(&nl, fin *1000);

    // log relative time
    fprintf(log, "%lf %zd %lf %zd\n", timeAsDouble() - starttime, *iopos, fin, run);
    
    if (ret < 0) {
      perror(readtest ? "READ" : "WRITE");
      exit(1);
    }
    ios++;
    //    fprintf(stderr, "%zd\n", testblock);
  }
  double iops = ios / (end - start);
  fprintf(stderr,"*info* %s %s blocksize=%7zd: %6zd IOs in %6.3lf seconds (99%% <= %6.1lf ms, s.d. %5.1lf ms), is %6.0lf IO/s, %4.0lf MB/s\n",
	  seq ? "Seq" : "Rnd", readtest ? "READ " : "WRITE", blocksz, ios, end - start, nlSortedPos(&nl, 0.99), nlSD(&nl), iops, iops * blocksz/1000/1000);
  nlFree(&nl);

  if (readtest) {
    free(readmem);
  }
}
      
      
      
      
void usage(void) {
  printf("usage: testdevice [ -b | -p ] {device} ... {device}\n");
  printf("\n");
  printf("options:\n");
  printf("   -b    # benchmark device\n");
  printf("   -p    # write persistence test\n");
}  


int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;

  srand48(time(NULL));

  int opt;
  int benchmark = 0, persistence = 0;
  
  while ((opt = getopt(argc, argv, "bp")) != -1) {
    switch (opt) {
    case 'b':
      benchmark = 1;
      persistence = 0;
      break;
    case 'p':
      persistence = 1;
      benchmark = 0;
      break;
    default:
      usage();
      exit(EXIT_FAILURE);
    }
  }

  if (benchmark + persistence == 0) {
    usage();
    exit(EXIT_FAILURE);
  }


  double testtime = 0.5;
  
  

  int first = 0;
  for (; optind < argc; optind++) {
    size_t bytes = 0;
    const char *device = argv[optind];
  
    //    const char *suffix = getSuffix(device);
    
    size_t sz = 1024 * 1024 * 100;
    
    
    int fd = open(device, O_RDWR | O_EXCL | O_DIRECT);
    if (fd >= 0) {
      char *v = NULL, *m = NULL;
      double bdGB = blockDeviceSizeFromFD(fd)/1000.0/1000.0/1000.0;

      fprintf(stderr,"*info* '%s': %s %s, %.0lf GB\n", device, v=vendorFromFD(fd), m=modelFromFD(fd), bdGB);

      char s[255];
      sprintf(s, "%s %s %.0lf GB", v, m, bdGB);
      FILE *datafile = fopen(s, "wt");
      if (datafile == NULL) {
	perror(s);
	exit(EXIT_FAILURE);
      }
      
      
      
      free(v); free(m);

      char *serial = serialFromFD(fd);
      if (serial == NULL) {
	fprintf(stderr,"*sorry* device %s has no serial, so I can't look for it once reinserted\n", device);
	exit(1);
      }

      fprintf(stderr,"*info* opened '%s' (%s)\n", device, serial);
      void *p = readData(fd, sz);

      if (benchmark) {

	FILE *gnuplot = fopen("testdevice.gplot", "wt");
	if (gnuplot == NULL) {
	  perror("testdevice.gplot");
	  exit(EXIT_FAILURE);
	}
	fprintf(gnuplot, "set title 'Block Device Comparison'\n");
	fprintf(gnuplot, "set xlabel 'Time taken for tests'\n");
	fprintf(gnuplot, "set ylabel 'Bytes processed'\n");
	fprintf(gnuplot, "plot ");
      
  // start
	double starttime = timeAsDouble();
	
	size_t run = 0;
	
	for (int seq = 0; seq <= 1; seq++) {
	  for (int rw = 0; rw <= 1; rw++) {
	    do_benchmark(fd, p, sz, 4096, testtime, rw, seq, datafile, &bytes, starttime, optind + run++);
	    do_benchmark(fd, p, sz, 65536, testtime, rw, seq, datafile, &bytes, starttime, optind + run++);
	    do_benchmark(fd, p, sz, 1024*1024, testtime, rw, seq, datafile, &bytes, starttime, optind + run++);
	  }
	}
	
	fclose(datafile);
	checkData(fd, p, sz);
	close(fd);
	free(p);
	p = NULL;
	
	if (first++ == 0) {
	  fprintf(gnuplot, ", ");
	}
	
	fprintf(gnuplot, "\"%s\" u 1:2 with lines lw 5", s);
	fflush(gnuplot);
	// end benchmark
	fprintf(stderr,"*info* test passed.\n");

	fprintf(gnuplot, "\n");
	fclose(gnuplot);

      } else {
	// persistence check
	const int maxblocks = (sz / 4096) - 1;
	char *buf = NULL;
	int retalloc = posix_memalign((void*)&buf, 4096, 4096); assert(buf);
	if (retalloc != 0) {
	  fprintf(stderr,"*can't alloc\n");
	  exit(EXIT_FAILURE);
	}
	for (size_t i = 0; i < 4096; i++) {
	  buf[i] = 'A'+(lrand48() % 26);
	}

	size_t startSeed = timeAsDouble()*100, thisSeed = 0, lastGoodSeed = 0;

	if (maxblocks > 0) {
	  int error = 0;
	  srand48(startSeed);
	  double lasttime =0;
	  int firstwrite = 1;
	  
	  fprintf(stderr,"*info* writing to %s\n", serial);
	  while (error == 0) {
	    thisSeed = lrand48();
	    size_t pos = (thisSeed % maxblocks) * 4096;
	    assert(pos < sz);
	    if (clobberData(fd, pos, 4096, buf) < 0) {
	      error = 1;
	      break;
	    }
	    lastGoodSeed = thisSeed;
	    if (firstwrite) {
	      firstwrite = 0;
	      fprintf(stderr,"*info* remove the device from now on...\n");
	    }
	    double thistime = timeAsDouble();
	    if (thistime - lasttime >= 1) {
	      fprintf(stderr,"."); fflush(stderr);
	      lasttime = thistime;
	    }
	  } // write heaps
	  //	  fprintf(stderr,"I/O error with '%s', serial '%s'\n", device, serial);
	  //	  perror(device);


	  fprintf(stderr,"\n*info* The last seed used was %zd\n", lastGoodSeed);
	  fprintf(stderr,"*info* Please reinsert the device...\n");
	  int foundserial = 0;
	  char *newdev = NULL;
	  while (foundserial == 0) {
	    blockDevicesType *bd = blockDevicesInit();
	    blockDevicesScan(bd);
	    newdev = blockDeviceFromSerial(bd, serial);
	    if (newdev) {
	      // reinserted
	      foundserial = 1;
	    }
	    if (foundserial == 0) {
	      fprintf(stderr, "."); fflush(stderr);
	    }
	    sleep(1);
	  }

	  close(fd);
	  fprintf(stderr,"\n*info* device %s has been reinserted as '%s'\n", serial, newdev);
	  fprintf(stderr,"*info* setting seed back to the last seed of %zd, was the block written? \n", lastGoodSeed);
	  fd = open(newdev, O_RDWR | O_EXCL | O_DIRECT);
	  if (fd < 0) {
	    perror(newdev);
	    fprintf(stderr,"*error* device is there, but we can't open?\n");
	    exit(1);
	  }
	  while (serialFromFD(fd) == NULL) {
	    fprintf(stderr,"*info* waiting on udev\n");
	    sleep(1);
	  }
	  
	  checkDidWrite(fd, buf, 4096, (lastGoodSeed % maxblocks) * 4096);

	  //	  checkData(fd, p, sz);
	  free(p);
	  p = NULL;
	}
	close(fd);
      }
    } else {
      perror(device);
    }
  }
  
  return 0;
}

