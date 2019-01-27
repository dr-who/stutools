#define _POSIX_C_SOURCE 200809L


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

  char *buffer, *loadblock;
  CALLOC(buffer, pc.maxbs+1, 1);
  CALLOC(loadblock, pc.maxbs+1, 1);

  FILE *fp = fopen(pc.device, "r");
  if (!fp) {perror(pc.device);exit(-2);}

  size_t correct = 0, incorrect = 0, lastseed = -1, uuid = -1, wrongpos = 0, wronguuid = 0;
  size_t printed = 0;
  for (size_t i = 0; i < pc.sz; i++) {
    if (pc.positions[i].action == 'W') {
      //      fprintf(stderr,"[%zd] %zd %c\n", i, pc.positions[i].pos, pc.positions[i].action);
      fseek(fp, pc.positions[i].pos, SEEK_SET);
      ssize_t ret = fread(loadblock, 1, pc.positions[i].len, fp);
      if (ret!=pc.positions[i].len) {perror("eerk");exit(-1);}

      if (pc.positions[i].seed != lastseed) {
	generateRandomBuffer(buffer, pc.maxbs, pc.positions[i].seed);
	lastseed = pc.positions[i].seed;
      }
      // check position
      size_t *p = (size_t*)loadblock;
      if (*p != pc.positions[i].pos) {
	if (printed++ < 5) 
	  fprintf(stderr,"position %zd had incorrect stored position of %zd\n", pc.positions[i].pos, *p);
	wrongpos++;
	continue;
      }

      if (uuid == -1) {
	size_t *u = (p+1);
	uuid = *u;
      } else {
	size_t *u = (p+1);
	if (*u != uuid) {
	  if (printed++ < 5) 
	    fprintf(stderr,"inconsistent thread uuid at block %zd\n", pc.positions[i].pos);
	  wronguuid++;
	  continue;
	}
      }
	
      
      if (strcmp(loadblock+16, buffer+16)==0) {
	correct++;
      } else {
	if (printed++ < 5) 
	  fprintf(stderr,"[%zd, %zd]\n1: %s\n2: %s\n", *p, pc.positions[i].pos, loadblock+16, buffer+16);
	incorrect++;
      }
    }
  }

  fclose(fp);

  fprintf(stderr,"*info* total %zd, correct %zd, incorrect %zd, wrong stored pos %zd, wrong thread uuid %zd\n", correct+incorrect+wrongpos+wronguuid, correct, incorrect, wrongpos, wronguuid);


  exit(0);
}
  
  
