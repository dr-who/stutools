#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#include <getopt.h>

#include "utils.h"

int keepRunning = 1;

void usage(void) {
    fprintf(stdout, "Usage:\n");
    fprintf(stdout, "  genpw\n");
    fprintf(stdout, "\n");
}

int main(int argc, char *argv[]) {
  (void) argc;
  (void) argv;
  
  const size_t len = 32;

  unsigned char *bits = randomGenerate(len);
  unsigned char *pw = passwordGenerate(bits, len);

  double entropy = entropyTotalBytes(pw, len);

  fprintf(stdout, "%s (%.1lf bits of entropy)\n", pw, entropy);

  free(pw);
  free(bits);

  return 0;
}
