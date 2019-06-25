#define _XOPEN_SOURCE 500
   
#include <stdlib.h>
#include <stdio.h>

#include "string.h"
#include "utils.h"
#include "fuzz.h"


char *randomCommandString(const double rwratio) {
  char string[1000];
  
  int s = lrand48() % 5;
  switch (s) {
  case 0: 
    sprintf(string, "%cs%ld", (drand48() < rwratio) ? 'r' : 'w', lrand48() % 100);
    break;
  case 1: 
    sprintf(string, "%cP%ld%c", (drand48() < rwratio) ? 'r' : 'w', 1+lrand48() % 10000, (drand48() < 0.5) ? 'n' : 'N');
    break;
  case 2: 
    sprintf(string, "%cP%ldx%ld", (drand48() < rwratio) ? 'r' : 'w', 1+lrand48() % 10000, 1 + lrand48()%100);
    break;
  case 3: 
    sprintf(string, "%ck%ld", (drand48()< rwratio) ? 'r' : 'w', 4 * (1+ (lrand48() % 4)));
    break;
  case 4: {}
    size_t klow = 4 * (1 + (lrand48() % 3));
    sprintf(string, "%ck%ld-%ld", (drand48() < rwratio) ? 'r' : 'w', klow, klow + 4*(lrand48()%4));
    break;
  }
  return strdup(string);
}

  

char ** fuzzString(int *argc, const char *device) {
  srand48(timedouble());
  size_t count = 1 + lrand48() % 3;
  count = 8;

  *argc = count;
  char ** argv = NULL;
  CALLOC(argv, count, sizeof(char*));

  char string[100];

  argv[0] = strdup("spit");
  sprintf(string, "-f%s", device);
  argv[1] = strdup(string);

  argv[2] = strdup("-G");
  double low = 0.1 + drand48() * 5;
  if (drand48() < 0.25) {
    sprintf(string, "%f", low);
  } else {
    sprintf(string, "%f-%f", low, 0.1 + low + drand48() * 30);
  }
    
  argv[3] = strdup(string);

  argv[4] = strdup("-c");
  argv[5] = randomCommandString(0.5);

  if (drand48() < 0.5) {
    argv[6] = strdup("-j");
    sprintf(string, "%zd", 1+lrand48() % 8);
    argv[7] = strdup(string);
  } else {
    argv[6] = strdup("-c");
    argv[7] = randomCommandString(0.5);
  }

  fprintf(stderr,"*info* random command: ");
  for (size_t i = 0; i < count; i++) {
    fprintf(stderr,"%s ", argv[i]);
  }
  fprintf(stderr,"\n");

  return argv;
}
