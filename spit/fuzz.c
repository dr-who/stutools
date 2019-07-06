#define _XOPEN_SOURCE 500
   
#include <stdlib.h>
#include <stdio.h>

#include "string.h"
#include "utils.h"
#include "fuzz.h"


char *randomCommandString(const double rwratio) {
  char string[1000];
  
  int seed = lrand48() % 65536;
  int s = lrand48() % 7;
  switch (s) {
  case 0: 
    sprintf(string, "p%.1lfs%ldR%d", rwratio, lrand48() % 100, seed);
    break;
  case 1: 
    sprintf(string, "p%.1lfP%ld%cR%d", rwratio, 1+lrand48() % 10000, (drand48() < 0.5) ? 'n' : 'N', seed);
    break;
  case 2: 
    sprintf(string, "p%.1lfP%ldx%ldR%d", rwratio, 1+lrand48() % 10000, 1 + lrand48()%100, seed);
    break;
  case 3: 
    sprintf(string, "p%.1lfk%ldR%d", rwratio, 4 * (1+ (lrand48() % 4)), seed);
    break;
  case 4: {}
    size_t klow = 4 * (1 + (lrand48() % 3));
    sprintf(string, "p%.1lfk%ld-%ldR%d", rwratio, klow, klow + 4*(lrand48()%4), seed);
    break;
  case 5:  case 6:
    sprintf(string, "mP%ld%cR%d", 1+lrand48() % 10000, (drand48() < 0.5) ? 'n' : 'N', seed);
    break;
  }
  return strdup(string);
}

  

char ** fuzzString(int *argc, const char *device) {
  size_t count = 1 + lrand48() % 3;
  count = 9;

  *argc = count;
  char ** argv = NULL;
  CALLOC(argv, count, sizeof(char*));

  char string[1000];

  argv[0] = strdup("spit");
  sprintf(string, "-f%s", device);
  argv[1] = strdup(string);

  argv[2] = strdup("-G");
  double low = 0.1 + drand48() * 5;
  if (drand48() < 0.25) {
    sprintf(string, "%f", low);
  } else {
    sprintf(string, "%f-%f", low, 0.1 + low + drand48() * 1);
  }
    
  argv[3] = strdup(string);

  argv[4] = strdup("-c");
  double d = drand48(); // r, w or r/w
  if (d < 0.4) {
    argv[5] = randomCommandString(0);
  } else if (d < 0.8) {
    argv[5] = randomCommandString(1);
  } else {
    argv[5] = randomCommandString(0.5);
  }
    

  d = drand48();
  
  if (d < 0.5) { // 50 of the time it's one
    argv[6] = strdup("-j");
    argv[7] = strdup("1");
  } else if (d < 0.9) { // 40% of the time it's another string
    argv[6] = strdup("-c");
    argv[7] = randomCommandString(0.9);
  } else { // remaining 10% multiple j
    argv[6] = strdup("-j");
    sprintf(string, "%zd", 1+lrand48() % 64);
    argv[7] = strdup(string);
  }

  sprintf(string,"-v");
  argv[8] = strdup(string); // verify

  fprintf(stderr,"====================\n");
  fprintf(stderr,"*info* random command: ");

  for (size_t i = 0; i < count; i++) {
    fprintf(stderr,"%s ", argv[i]);
  }
  fprintf(stderr,"\n");
  fprintf(stderr,"====================\n");

  return argv;
}
