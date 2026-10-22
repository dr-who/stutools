#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#define BLOCKSIZE (2*1024*1024)



void replayBlock(int fd, unsigned char *buf, size_t len, size_t offset, unsigned char *readonlybuf) {
  pread(fd, readonlybuf, len, offset);

  if (memcmp(buf, readonlybuf, len) == 0) {
    fprintf(stderr,"*error* the destination already has this block\n");
  }
  
  pwrite(fd, buf, len, offset);
}


void usage() {
  fprintf(stderr,"*usage* blockdevreplay /dev/older < delta.0001\n");
  fprintf(stderr,"*usage* cat delta.00?? | blockdevreplay /dev/older    # replay in glob order\n");
}


int main(int argc, char *argv[]) {

  if (argc < 2) {
    usage();
    exit(1);
  }
  
  int fd1 = open(argv[1], O_RDWR);
  if (fd1 < 0) {
    perror(argv[1]);
    exit(1);
  }

  unsigned char *buf = malloc(4096); assert(buf);
  unsigned char *readonlybuf = malloc(4096); assert(readonlybuf);
  ssize_t read1 = 0;
  size_t offset = 0;
  size_t blocks = 0;
  
  while ( (read1=read(0, &offset, sizeof(size_t))) > 0) {
    //    fprintf(stderr,"*position diff* %zd\n", offset);
    read1 = read(0, buf, 4096);
    if (read1 == 4096) {
      replayBlock(fd1, buf, 4096, offset, readonlybuf);
      blocks++;
    }
  }
  close(fd1);
  fprintf(stderr,"*info* %zd blocks updated\n", blocks);

  free(buf);
  free(readonlybuf);
  
  exit(0);
}

    
  
