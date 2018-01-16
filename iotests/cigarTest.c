#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <sys/time.h>

#include "utils.h"
#include "cigar.h"



struct timeval gettm() {
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
}

double timedouble() {
  struct timeval now = gettm();
  double tm = (now.tv_sec * 1000000 + now.tv_usec);
  return tm/1000000.0;
}

int main(int argc, char *argv[]) {
  srand48(timedouble() * 1000);
  if (argc < 2) {
    fprintf(stderr,"./cigarTest 10.20-30X\n");
    fprintf(stderr,"./cigarTest 100R\n");
    fprintf(stderr,"./cigarTest 100R_100W_\n");
    fprintf(stderr,"./cigarTest ~R          # ~ means a random number [1, 10]\n");
    fprintf(stderr,"./cigarTest @R          # @ means a random number [11, 100]\n");
    fprintf(stderr,"./cigarTest :R          # : means a random number [100, 1000]\n");
    fprintf(stderr,"./cigarTest @X\n");
    fprintf(stderr,"./cigarTest ~R1f@W1f:X1f\n");
  } else {
    cigartype c;
    cigar_init(&c);
    
    cigar_setrwrand(&c, 0.5);
    if (cigar_parse(&c, argv[1]) == 0) {
      cigar_dump(&c, stdout);
      fputc('\n', stdout);
    }
    
    cigar_free(&c);
  }
  
  return 0;
}
