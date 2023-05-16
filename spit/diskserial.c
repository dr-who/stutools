#include <stdlib.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#include "procDiskStats.h"

#include "diskStats.h"
#include "utils.h"

int keepRunning = 1;

int main() {
  char s[PATH_MAX];
  
  while (fgets(s, PATH_MAX-1, stdin) != NULL) {
    s[strlen(s)-1] = 0;
    int fd = open(s, O_RDONLY);
    if (fd > 0) {
      char *s = serialFromFD(fd);
      printf("%s\n", s);
      free(s);
      close(fd);
    } else {
      perror(s);
    }
  }
  
  exit(0);
}

  
