#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#include <getopt.h>

#include "utils.h"

int keepRunning = 1;

void usage() {
  fprintf(stdout,"Usage:\n");
  fprintf(stdout,"  genpw\n");
  fprintf(stdout,"\n");
}

int main()
{
  const size_t len = 32;

  unsigned char *pw = passwordGenerate(len);

  double entropy = entropyTotalBits(pw, len, 1);
  
  fprintf(stdout, "%s (%.1lf bits of entropy)\n", pw, entropy);

  free(pw);
  
  return 0;
}
