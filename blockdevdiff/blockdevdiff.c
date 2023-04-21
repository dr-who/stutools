#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#define BLOCKSIZE (2*1024*1024)


void diffTwoBDs(int fd1, int fd2) {

  size_t blocks = 0;

  size_t pos1 = 0;
  size_t pos2 = 0;

  size_t nextPrint = pos1 + (1024L*1024*1024);
  
  unsigned char *buf1 = (unsigned char*)malloc(BLOCKSIZE); assert(buf1);
  unsigned char *buf2 = (unsigned char*)malloc(BLOCKSIZE); assert(buf2);

  while (1) {
    if (pos1 >= nextPrint) {
      fprintf(stderr,"*info* up to %.1lf GB\n", pos1/1024.0/1024/1024);
      nextPrint = pos1 + (1024L*1024*1024);
    }

    ssize_t read1 = read(fd1, buf1, BLOCKSIZE);
    ssize_t read2 = read(fd2, buf2, BLOCKSIZE);

    if (read1 != read2) {
      fprintf(stderr,"*error* file sizes are different. Delta is invalid");
      exit(1);
    }

    if (read1 <= 0) {
      fprintf(stderr,"*info* EOF at %ld\n", pos1 + read1);
      break; // eof
    }

    if (memcmp(buf1, buf2, BLOCKSIZE) == 0) {
      // same
    } else {
      for (size_t i = 0; i < BLOCKSIZE; i += 4096) {
	if (memcmp(buf1 + i, buf2 + i, 4096) != 0) {
	  size_t pos = pos1 + i;
	  //	  fprintf(stderr,"*info* difference at position %ld (%.1lf GB), size %d\n", pos, pos/1024.0/1024/1024, 4096);
	  write(1, &pos, sizeof(size_t));
	  write(1, buf2 + i, 4096);
	  blocks++;
	}
      }
    }

    pos1 += read1;
    pos2 += read2;
  }
  
  free(buf1);
  free(buf2);
  fprintf(stderr,"*info* %zd blocks in the delta, size of %zd x (4096 + %zd) = %zd\n", blocks, blocks, sizeof(size_t), blocks * (4096 + sizeof(size_t)));
}


void usage() {
  fprintf(stderr,"*usage* blockdevdiff /dev/older /dev/newer >delta\n");
}

int main(int argc, char *argv[]) {

  if (argc < 3) {
    usage();
    exit(1);
  }

  int fd1 = open(argv[1], O_RDONLY);
  if (fd1 < 0) {
    perror(argv[1]);
    exit(1);
  }

  int fd2 = open(argv[2], O_RDONLY);
  if (fd2 < 0) {
    perror(argv[2]);
    exit(1);
  }

  diffTwoBDs(fd1, fd2);

  close(fd1);
  close(fd2);

  exit(0);
}

    
  
