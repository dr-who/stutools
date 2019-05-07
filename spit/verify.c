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

// sorting function, used by qsort
static int seedcompare(const void *p1, const void *p2)
{
  const positionType *pos1 = (positionType*)p1;
  const positionType *pos2 = (positionType*)p2;
  if (pos1->seed < pos2->seed) return -1;
  else if (pos1->seed > pos2->seed) return 1;
  else {
    if (pos1->pos < pos2->pos) return -1;
    else if (pos1->pos > pos2->pos) return 1;
    else return 0;
  }
}

/**
 * main
 *
 */
int main(int argc, char *argv[]) {

  // load in all the positions, generation from the -L filename option from aioRWTest

  if (argc <= 1) {
    fprintf(stderr,"*usage* ./verify spit*.txt\n");
    exit(1);
  }
  
  size_t numFiles = argc -1;

  positionContainer *origpc;
  CALLOC(origpc, numFiles, sizeof(positionContainer));

  for (size_t i = 0; i < numFiles; i++) {
    fprintf(stderr,"*info* position file: %s\n", argv[1 + i]);
    FILE *fp = fopen(argv[i+1], "rt");
    if (fp) {
      positionContainerLoad(&origpc[i], fp);
      positionContainerInfo(&origpc[i]);
      fclose(fp);
    }
  }

  positionContainer pc = positionContainerMerge(origpc, numFiles);

  //  fprintf(stderr,"*info* combined file: '%s'\n", pc.device);
  fprintf(stderr,"*info* starting verify\n");

  //  positionContainerSave(&mergedpc, "test", mergedpc.bdSize, 0);
  char *loadblock= NULL;
  char *buffer = NULL;
  
  CALLOC(buffer, pc.maxbs+1, 1);
  CALLOC(loadblock, pc.maxbs+1, 1);

  int fd = 0;
  if (pc.sz) {
    fd = open(pc.device, O_RDONLY | O_DIRECT);
    if (fd < 0) {perror(pc.device);exit(-2);}
  }

  size_t correct = 0, incorrect = 0;
  unsigned short lastseed = 0;
  size_t wrongpos = 0, wronguuid = 0;
  size_t printed = 0;

  size_t p, tested = 0;

  qsort(pc.positions, pc.sz, sizeof(positionType), seedcompare);

  
  for (size_t i = 0; i < pc.sz; i++) {
    tested++;
    if (pc.positions[i].action == 'W') {
      //      fprintf(stderr,"[%zd] %zd %c %d\n", i, pc.positions[i].pos, pc.positions[i].action, pc.positions[i].seed);
      if(lseek(fd, pc.positions[i].pos, SEEK_SET) < 0) {
	fprintf(stderr,"problem\n");
      }
      
      ssize_t ret = read(fd, loadblock, pc.positions[i].len);
      if (ret != pc.positions[i].len) {perror("eerk");exit(-1);}

      if (pc.positions[i].seed != lastseed || i==0) {
	fprintf(stderr,"*info* generating %zd buffer for seed %d\n", pc.maxbs, pc.positions[i].seed);
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

      if (strncmp((char*)(loadblock+16), (char*)(buffer+16), pc.minbs-16-2)==0) {
	correct++;
      } else {
	if (printed++ <= 10)  {
	  fprintf(stderr,"[%zd, %zd, seed %d]\n", p, pc.positions[i].pos, lastseed);
	  char s[1000];
	  strncpy(s, loadblock+16, 1000);
	  s[999]=0;
	  fprintf(stderr,"1: %s\n", s);
	  strncpy(s, buffer+16, 1000);
	  s[999]=0;
	  fprintf(stderr,"2: %s\n", s);
	}
	incorrect++;
      }
    }
  }

  close(fd);

  fprintf(stderr,"*info* total %zd, correct %zd, incorrect %zd, wrong stored pos %zd, wrong thread uuid %zd\n", correct+incorrect+wrongpos+wronguuid, correct, incorrect, wrongpos, wronguuid);

  free(buffer); buffer=NULL;
  free(loadblock); loadblock=NULL;


  for (size_t i = 0; i < numFiles; i++) {
    free(origpc[i].device);
    free(origpc[i].string);
    positionContainerFree(&origpc[i]);
  }
  free(origpc); origpc=NULL;

  positionContainerFree(&pc);

  if (tested && (incorrect + wrongpos)) {
    exit(1);
  } else {
    exit(0);
  }
}
  
  
