#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <assert.h>
#include <errno.h>
#include <string.h>
#include <stdio.h>
#include <sys/mman.h>
#include "utils.h"

int keepRunning = 1;
int verbose = 0;

int main(int argc, char *argv[])
{
  size_t sz = 1024*1024*32;
  if (argc < 3) {
    fprintf(stderr,"Usage: ./pmemwreck /dev/dax0.0 sizeInBytes (say $[32*1024*1024])\n");
    exit(1);
  }
  const char* dev = argv[1];
  sz = atol(argv[2]);
  fprintf(stderr,"*info* opening '%s' (first %zd bytes), wrecking a byte every 4096\n", dev, sz);

  char sys[1000], *suffix = getSuffix(dev);
  sprintf(sys, "/sys/bus/dax/devices/%s/size", suffix);
  FILE *fp = fopen(sys, "rt"); assert(fp);
  size_t maxsz = 0;
  int ret = fscanf(fp, "%lu", &maxsz);
  if (ret != 1) maxsz = 0;
  fprintf(stderr,"*info* max size of device %zd\n", maxsz);
  if (sz > maxsz) {
    sz = maxsz;
  }

  int fd = open( dev, O_RDWR );
  if( fd < 0 ) {
    perror(dev);
    return 1;
  }

  void * src =  mmap (0, sz, PROT_WRITE, MAP_SHARED, fd, 0);
  if (!src) {
    fprintf(stderr,"*error* mmap failed\n");
    exit(1);
  }
  unsigned char *s = (unsigned char*)src;
  size_t changes = 0;
  for (size_t i = 0; i < sz; i += 4096) {
    //      fprintf(stderr,"[%zd] changed %d to %d\n", i, s[i], 255^s[i]);
    s[i] = 255 ^ s[i];
    changes++;
  }
  msync(src, sz, MS_SYNC);
  munmap(0, sz);


  fprintf(stderr,"*info* %zd changes applied to devices %s\n", changes, dev);
    
  close( fd );
  return 0;
}
