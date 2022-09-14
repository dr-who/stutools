#include <stdlib.h>
#include <stdio.h>

#include "procDiskStats.h"

#include "diskStats.h"
#include "utils.h"

int keepRunning = 1;

int main() {

  unsigned int major, minor;
  char s[1001];
  
  while (fgets(s, 1000, stdin) != NULL) {
    s[strlen(s)-1] = 0;
    majorAndMinorFromFilename(s, &major, &minor);
    char *serial = getFieldFromUdev(major, minor, "E:ID_SERIAL=");
    fprintf(stdout,"%s\t%s\n", s, serial);
    free(serial);
  }
  
  exit(0);
}

  
