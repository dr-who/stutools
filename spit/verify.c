#define _POSIX_C_SOURCE 200809L
#define _GNU_SOURCE

#include <fcntl.h>
#include <sys/types.h>
#include <sys/stat.h>

/**
 * verify.c
 *
 * the main() for ./verify, a standalone position verifier
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"
  
int verbose = 1;
int keepRunning = 1;
size_t waitEvery = 0;    
/**
 * main
 *
 */
int main(int argc, char *argv[]) {

  // load in all the positions, generation from the -L filename option from aioRWTest

  positionContainer pc;
  
  positionContainerLoad(&pc, stdin);
  positionContainerInfo(&pc);

  size_t sum = 0;
  for (size_t i = 0; i < pc.sz; i++) {
    sum += pc.positions[i].len;
  }
  fprintf(stderr,"device covered %zd bytes (%.3lf GiB)\n", sum, TOGiB(sum));

  char *loadblock= NULL;
  char *buffer = NULL;
  
  CALLOC(buffer, pc.maxbs+1, 1);
  CALLOC(loadblock, pc.maxbs+1, 1);


  int fd = open(pc.device, O_RDONLY| O_DIRECT);
  if (fd < 0) {perror(pc.device);exit(-2);}

  size_t correct = 0, incorrect = 0;
  unsigned short lastseed = 0;
  size_t wrongpos = 0, wronguuid = 0;
  size_t printed = 0;

  size_t p;
  
  for (size_t i = 0; i < pc.sz; i++) {
    if (pc.positions[i].action == 'W') {
      //      fprintf(stderr,"[%zd] %zd %c %d\n", i, pc.positions[i].pos, pc.positions[i].action, pc.positions[i].seed);
      lseek(fd, pc.positions[i].pos, SEEK_SET);
      ssize_t ret = read(fd, loadblock, pc.positions[i].len);
      if (ret != pc.positions[i].len) {perror("eerk");exit(-1);}

      if (pc.positions[i].seed != lastseed || i==0) {
	generateRandomBuffer(buffer, pc.maxbs, pc.positions[i].seed);
	lastseed = pc.positions[i].seed;
      }
      // check position
      memcpy(&p, loadblock, sizeof(size_t));
      if (p != pc.positions[i].pos) {
	if (printed++ <= 10) 
	  fprintf(stderr,"position %zd had incorrect stored position of %zd\n", pc.positions[i].pos, p);
	wrongpos++;
	continue;
      }

      if (strcmp((char*)(loadblock+16), (char*)(buffer+16))==0) {
	correct++;
      } else {
	if (printed++ <= 10) 
	  fprintf(stderr,"[%zd, %zd]\n1: %s\n2: %s\n", p, pc.positions[i].pos, loadblock+16, buffer+16);
	incorrect++;
      }
    }
  }

  close(fd);

  fprintf(stderr,"*info* total %zd, correct %zd, incorrect %zd, wrong stored pos %zd, wrong thread uuid %zd\n", correct+incorrect+wrongpos+wronguuid, correct, incorrect, wrongpos, wronguuid);


  exit(0);
}
  
  
