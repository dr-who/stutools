#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>
#include <time.h>


void usage() {
  printf("usage: modcat [options] < stdin > stdout\n");
  printf("\n");
  printf("  -m num   # print the line if (linenofromzero %% num) == 0\n");
  printf("  -c       # also output stderr progress\n");
  exit(-1);
}
  
int main(int argc, char *argv[]) {
  
  char *line = NULL;
  size_t len = 0;
  ssize_t nread;

  int mod = 1, opt;
  int progress = 0;

  while ((opt = getopt(argc, argv, "cm:")) != -1) {
    switch (opt) {
    case 'm':
      mod = atoi(optarg);
      if (mod < 1) mod = 1;
      //      fprintf(stderr,"*info* print every %d lines\n", mod);
      break;
    case 'c':
      progress = 1;
      break;
    default:
      usage();
      break;
    }
  }


  int count = 0;
  char outstr[1000];
  
  while ((nread = getline(&line, &len, stdin)) != -1) {
    if ((mod == 1) || ((count % mod) == 0)) {
      fwrite(line, nread, 1, stdout);
    }
    count++;

    if (progress) {
      
      time_t t;
      struct tm *tmp;
      
      t = time(NULL);
      tmp = localtime(&t);
      if (tmp == NULL) {
	perror("localtime");
	exit(EXIT_FAILURE);
      }
      
      if (strftime(outstr, sizeof(outstr), "%c %Z" , tmp) == 0) {
	fprintf(stderr, "strftime returned 0");
	exit(EXIT_FAILURE);
      }
      
      fprintf(stderr,"[%s] processed %d lines\n", outstr, count);
    }
    
  }
  
  free(line);
  exit(EXIT_SUCCESS);
}
