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

#define ALIGN 2097152

int main(int argc, char *argv[])
{
  size_t sz = 0;
  if (argc < 2) {
    fprintf(stderr,"Usage: ./pmemwreck /dev/dax0.0 [sizeInBytes (say $[32*1024*1024])]\n");
    exit(1);
  }
  const char* dev = argv[1];
  if (argc == 3) {
    sz = alignedNumber(atol(argv[2]), ALIGN);
  }

  char sys[1000], *suffix = getSuffix(dev);
  sprintf(sys, "/sys/bus/dax/devices/%s/size", suffix);
  FILE *fp = fopen(sys, "rt"); assert(fp);
  size_t maxsz = 0;
  int ret = fscanf(fp, "%lu", &maxsz);
  maxsz = alignedNumber(maxsz, ALIGN);
  if (ret != 1) maxsz = 0;
  if (sz > maxsz) {
    sz = maxsz;
  }
  if (sz == 0) {
    sz = maxsz;
  }

  fprintf(stderr,"*info* logical size of device %zd (%.1lf GiB)\n", maxsz, TOGiB(maxsz));
  fprintf(stderr,"*info* opening '%s', wrecking %zd bytes (%.1lf GiB), in 4096 steps\n", dev, sz, TOGiB(sz));


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
