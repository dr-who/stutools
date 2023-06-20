#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>
#include <time.h>


void usage() {
  printf("usage: modcat [options] < stdin > stdout\n");
  printf("\n");
  printf("  -m num   # print the line if (line_no %% num) == 1\n");
  printf("  -d num   # print date every num lines\n");
  exit(-1);
}
  
int main(int argc, char *argv[]) {
  
  char *line = NULL;
  size_t len = 0;
  ssize_t nread;

  int mod = 1, opt;
  int progress = 0;

  while ((opt = getopt(argc, argv, "d:m:")) != -1) {
    switch (opt) {
    case 'm':
      mod = atoi(optarg);
      if (mod < 1) mod = 1;
      //      fprintf(stderr,"*info* print every %d lines\n", mod);
      break;
    case 'd':
      progress = atoi(optarg);
      if (progress < 1) progress = 1;
      break;
    default:
      usage();
      break;
    }
  }


  int count = 0;
  char outstr[1000];
  
  while ((nread = getline(&line, &len, stdin)) != -1) {
    count++;
    if ((mod == 1) || ((count % mod) == 1)) {
      fwrite(line, nread, 1, stdout);
    }
    
    if ((progress == 1) || ((count % progress) == 1)) {
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
