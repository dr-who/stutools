#define _XOPEN_SOURCE 500

#include <stdlib.h>
#include <stdio.h>
#include <time.h>
#include <assert.h>

#include "string.h"
#include "utils.h"
#include "spitfuzz.h"


char *randomCommandString(const double rwratio)
{
  char string[1000];

  int seed = lrand48() % 65536;
  int s = lrand48() % 11;
  switch (s) {
  case 0:
    sprintf(string, "p%.1lfs%ldR%d", rwratio, lrand48() % 100, seed);
    break;
  case 1:
    sprintf(string, "p%.1lfP%ldR%d", rwratio, 1+lrand48() % 10000, seed);
    break;
  case 2:
    sprintf(string, "p%.1lfP%ldx%ldR%d", rwratio, 1+lrand48() % 10000, 1 + lrand48()%100, seed);
    break;
  case 3:
    sprintf(string, "p%.1lfk%ldR%d", rwratio, 4 * (1+ (lrand48() % 4)), seed);
    break;
  case 4:
  {}
  size_t klow = 4 * (1 + (lrand48() % 3));
  sprintf(string, "p%.1lfk%zd-%ldR%d", rwratio, klow, klow + 4*(lrand48()%4), seed);
  break;
  case 5:
  case 6:
    sprintf(string, "mP%ldR%d", 1+lrand48() % 10000, seed);
    break;
  case 7:
  case 8:
  case 9:
    sprintf(string, "wk1024s1R%d", seed);
    break;
  case 10:
  {}
  klow = 4 * (1 + (lrand48() % 3));
  sprintf(string, "p%.1lfmk%zd-%ldR%d", rwratio, klow, klow + 16*(lrand48()%4), seed);
  break;
  default:
    abort();
  }

  return strdup(string);
}



char ** fuzzString(int *argc, const char *device, const double starttime, size_t *runcount)
{
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

  if (d < 0.75) { // 75% of the time it's one
    argv[6] = strdup("-j");
    argv[7] = strdup("1");
  } else if (d < 0.9) { // 15% of the time it's another string
    argv[6] = strdup("-c");
    argv[7] = randomCommandString(0.9);
  } else { // remaining 10% multiple j
    argv[6] = strdup("-j");
    sprintf(string, "%ld", 1+lrand48() % 10);
    argv[7] = strdup(string);
  }

  sprintf(string,"-v");
  argv[8] = strdup(string); // verify

  *runcount = (*runcount) + 1;
  fprintf(stderr,"====================\n");
  time_t now;
  time(&now);
  fprintf(stderr,"*info* run number %zd, running for %.2lf days, %s", *runcount, (timedouble() - starttime)/3600.0/24, ctime(&now));
  fprintf(stderr,"*info* random command: ");

  for (size_t i = 0; i < count; i++) {
    fprintf(stderr,"%s ", argv[i]);
  }
  fprintf(stderr,"\n");
  fprintf(stderr,"====================\n");

  return argv;
}
