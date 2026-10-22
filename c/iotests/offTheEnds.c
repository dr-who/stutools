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


int seekTest(int fd, const size_t pos, const size_t bdsize, const size_t len) {
  char *buf;

  fprintf(stderr,"*info* location %10zd / %9zd, size %6zd -- write\n", pos, bdsize, len);
  
  CALLOC(buf, len, sizeof(char));

  int ok = 0;
  int l = lseek(fd, pos, SEEK_SET);
  if (l < 0) {
    perror("lseek"); ok = 1;
  } else {
    
    int w = write(fd, buf, len);
    if (w != len) {
      perror("write"); ok = 1;
    }
  }
  
  free(buf);
  
  return ok;
}

void shouldbe(int val, int shouldbe) {
  if (val != shouldbe) {
    fprintf(stderr,"*error* !! this isnt right ^^ \n");
  }
}


int main(int argc, char *argv[]) {
  int index = 1;

  for (size_t i = index; i < argc; i++) {
    int fd = open(argv[i], O_RDWR | O_EXCL );
    if (fd >= 0) {
      size_t bdsize = blockDeviceSizeFromFD(fd);
      if (bdsize == 0) {
	fprintf(stderr,"*error* a zero byte device????\n");
	exit(-1);
      }
      fprintf(stderr,"*info* opened '%s', size %zd bytes (%.2lf GiB)\n", argv[i], bdsize, TOGiB(bdsize));
      //      trimDevice(fd, argv[i], 0, bdsize); // only sending 1GiB for now
      shouldbe(seekTest(fd, 0, bdsize, 65536), 0);
      shouldbe(seekTest(fd, 65536, bdsize, 65536), 0);
      shouldbe(seekTest(fd, bdsize-65536, bdsize, 65536), 0);
      shouldbe(seekTest(fd, bdsize-512, bdsize, 512), 0);
      shouldbe(seekTest(fd, bdsize-1, bdsize, 1), 0);
      shouldbe(seekTest(fd, bdsize, bdsize, 0), 0);
      shouldbe(seekTest(fd, bdsize+1, bdsize, 0), 1);
      shouldbe(seekTest(fd, bdsize, bdsize, 1), 1);
      shouldbe(seekTest(fd, bdsize, bdsize, 4096), 1);
      shouldbe(seekTest(fd, bdsize, bdsize, 65536), 1); 
      shouldbe(seekTest(fd, bdsize, bdsize, 1<<20), 1);    
      shouldbe(seekTest(fd, bdsize, bdsize, 1<<28), 1);
      shouldbe(seekTest(fd, 0, bdsize, 0), 0);
      shouldbe(seekTest(fd, 0, bdsize, bdsize+1), 1); 
      shouldbe(seekTest(fd, 0, bdsize, 1<<20), 0);
  //      shouldbe(seekTest(fd, bdsize, bdsize, 0);
      //      shouldbe(seekTest(fd, bdsize, bdsize, 65536);
      
      close(fd);
    } else {
      perror("open");
    }
  }

  return 0;
}
