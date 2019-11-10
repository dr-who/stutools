#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L
#define _XOPEN_SOURCE 500

#include "utils.h"

#include <stdio.h>
#include <stdlib.h>

#include <stdlib.h>
#include <malloc.h>
#include <unistd.h>

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

int keepRunning = 1;

void generate(unsigned char *buf, size_t size, unsigned int seed) {
  srand(seed);
  // interate in ints not chars
  unsigned int *p = (unsigned int*)buf;
  for (size_t i = 0; i < size / sizeof(unsigned int); i++) {
    *p = rand();
    p++;
  }
}



void verifyDevice(char *device, unsigned char *buf, size_t size) {
  
  unsigned char *readbuf = aligned_alloc(4096, size);
  if (!buf) {
    fprintf(stderr,"*error* can't allocate %zd bytes\n", size);
    exit(1);
  }
  memset(readbuf, 0, size);
  
  int fd = open(device, O_DIRECT | O_RDONLY | O_EXCL, S_IRUSR | S_IWUSR);
  if (fd >= 0) {
    unsigned char *p = readbuf;
    ssize_t ret = 0;
    size_t toreadleft = size;

    while ((ret = read(fd, p, MIN(1024*1024,toreadleft))) != 0) {
      //      fprintf(stderr,"*info read %zd %zd\n", p - readbuf, toreadleft);
      if (ret < 0) {
	fprintf(stderr,"*error* read problem\n");
	perror(device);
	exit(1);
      }
      toreadleft -= ret;
      p += ret;
    }
      
    if (toreadleft != 0) {
      fprintf(stderr,"*error* incomplete read size of %zd\n", size - toreadleft);
      exit(1);
    }
    
    // print out
    unsigned char str1[11], str2[11];
    memcpy(str1, buf, 10);
    memcpy(str2, readbuf, 10);
    str1[10]=0;
    str2[10]=0;

    int pc = memcmp(buf, readbuf, size);
    if (pc != 0) {
      fprintf(stderr,"*error* failed verification at offset %d\n", pc);
      exit(1);
    }
    fprintf(stderr,"*info* succeeded %zd byte verification of '%s'.\n*info* the first few bytes: RAM '%s', on device '%s'\n", size, device, str1, str2);
    close(fd);
  } else {
    perror(device);
    exit(1);
  }
}

void usage() {
  fprintf(stderr,"Usage: randdd -f /dev/device [-R seed] -G size (GiB) -v (verify)\n");
}

int main(int argc, char *argv[]) {

  int opt = 0, help = 0;
  unsigned int seed = 42;
  int verify = 0;
  size_t size = 16*1024*1024;
  char *device = NULL;
  
  while ((opt = getopt(argc, argv, "f:G:wvhR:")) != -1) {
    switch (opt) {
    case 'h':
      help = 1;
      break;
    case 'f':
      device = optarg;
      break;
    case 'R':
      seed = atoi(optarg);
      break;
    case 'G':
      size = (size_t)(atof(optarg) * 1024) * 1024 * 1024;
      fprintf(stderr,"*info* size %zd bytes (%.3lf GiB), RAM %zd bytes (%.0lf GiB)\n", size, TOGiB(size), totalRAM(), TOGiB(totalRAM()));
      if (size > totalRAM()) {
	fprintf(stderr,"*error* G ram is more than actual RAM\n");
	exit(1);
      }
      break;
    case 'v':
      verify = 1;
      break;
    case 'w':
      verify = 0;
      break;
    }
  }

  if (help || !device) {
    usage();
    exit(1);
  }

  unsigned char *buf = aligned_alloc(4096, size);
  if (!buf) {
    fprintf(stderr,"*error* can't allocate %zd bytes\n", size);
    exit(1);
  }
    
  generate(buf, size, seed);


  fprintf(stderr,"*info* randdd on '%s', seed %d, size %zd (%.3lf GiB), mode: %s\n", device, seed, size, TOGiB(size), verify?"VERIFY" : "WRITE");

  if (!verify) {
    int fd = open(device, O_DIRECT | O_WRONLY | O_TRUNC | O_EXCL, S_IRUSR | S_IWUSR);
    if (fd >= 0) {
      size_t towrite = size;
      unsigned char *p = buf;
      while (towrite > 0) {
	ssize_t written  = write(fd, p, MIN(towrite, 1024 * 1024));
	if (written <= 0) {
	  perror(device);
	  exit(1);
	}
	towrite -= written;
	p += written;
      }
      fdatasync(fd);
      close(fd);
      //
      fprintf(stderr,"*info* verifying prior write is on disk\n");
      verifyDevice(device, buf, size);
      fprintf(stderr,"*info* write stored OK\n");
    } else {
      perror(device);
      exit(1);
    }
  } else {
    verifyDevice(device, buf, size);
    fprintf(stderr,"*info* read verified OK\n");
  }

  free(buf);

}

