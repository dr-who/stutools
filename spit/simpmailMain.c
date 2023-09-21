#include <stdio.h>
#include <malloc.h>
#include <string.h>
#include <stdlib.h>
#include <getopt.h>

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

void usage() {
  printf("usage: simpmail <from> <to> <cc> <subject>   < file.html \n");
  printf("\nExample:\n");
  printf("  simpmail bob@example.com test@example.com cc@example.com \"Test subject in quotes\" < file.html \n");
  printf("\nHTML example\n");
  printf("  file.html:\n");
  printf("\n");
  printf("  <h1>Header</h1>\n");
  printf("  Welcome. <p>\n");
}

int main(int argc, char *argv[]) {

  int opt;
  char *fromemail = NULL, *fromname = NULL, *toemail = NULL, *ccemail = NULL, *bccemail = NULL, *subject = NULL;
  
  while ((opt = getopt(argc, argv, "f:F:t:c:b:s:")) != -1) {
    switch (opt) {
    case 'f':
      fromemail = strdup(optarg); break;
    case 'F':
      fromname = strdup(optarg); break;
    case 't':
      toemail = strdup(optarg); break;
    case 'c':
      ccemail = strdup(optarg); break;
    case 'b':
      bccemail = strdup(optarg); break;
    case 's':
      subject = strdup(optarg); break;
    default:
      fprintf(stderr,"unknown command\n");
      exit(EXIT_FAILURE);
    }
  }

  if (!fromemail || !toemail || !subject) {
    usage();
  } else {
    
    int fd = simpmailConnect("127.0.0.1");
    
    if (fd > 0) {
      char *body = readFile(stdin);
      simpmailSend(fd, fromemail, fromname, toemail, ccemail, bccemail, subject, body);
      simpmailClose(fd);
    }
  }

  return 0;
}
