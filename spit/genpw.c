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

  const char *buf="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";


  const int buflen = strlen(buf);
  
  for (size_t i = 0; i < 32; i++) {
    unsigned long l = getDevRandom();
    l = l % buflen;
    fprintf(stdout,"%c", buf[l]);
  }
  fprintf(stdout,"\n");
  
  return 0;
}
