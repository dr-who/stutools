#include <stdio.h>
#include <malloc.h>
#include <string.h>
#include <stdlib.h>

#include "simpmail.h"

// returns the number of lines
char *readFile(FILE *stream) {

  char *ret = calloc(1024*1024, 1);
  
    if (stream) {
        char *line = NULL;
        size_t linelen = 0;
        ssize_t nread;

        while ((nread = getline(&line, &linelen, stream)) != -1) {
            //      printf("Retrieved line of length %zu:\n", nread);

	  //	  len = len + (nread + 1);
	  
	  //	  ret = realloc(ret, len * sizeof(char));
	  
	  strcat(ret, line);
	  //	  strcat(ret, "\n");
        }

        free(line);
    } else {
        perror("problem");
    }

    return strdup(ret);
}



int main(int argc, char *argv[]) {
  if (argc < 4) {
    printf("usage: simpmail <from> <to> <cc> <subject>   < file.html \n");
    printf("\nExample:\n");
    printf("  simpmail bob@example.com test@example.com cc@example.com \"Test subject in quotes\" < file.html \n");
    printf("\nHTML example\n");
    printf("  file.html:\n");
    printf("\n");
    printf("  <h1>Header</h1>\n");
    printf("  Welcome. <p>\n");
    
    return 1;
  }

  int fd = simpmailConnect("127.0.0.1");

  if (fd > 0) {
    char *from = argv[1];
    char *to = argv[2];
    char *cc = argv[3];
    char *subject = argv[4];
    char *body = readFile(stdin);
    simpmailSend(fd, from, to, cc, subject, body);
    simpmailClose(fd);
  }

  return 0;
}
