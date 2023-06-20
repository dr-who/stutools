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
  printf("  -t secs  # print date every few secs\n");
  exit(-1);
}
  
int main(int argc, char *argv[]) {
  
  char *line = NULL;
  size_t len = 0;
  ssize_t nread;

  int opt;
  int mod = 1;      // -m option
  int progress = 0; // -d option
  int fewsecs = 0;  // -t option

  while ((opt = getopt(argc, argv, "d:m:t:")) != -1) {
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
    case 't':
      fewsecs = atoi(optarg);
      if (fewsecs < 0) fewsecs = 0;
      break;
    default:
      usage();
      break;
    }
  }


  int count = 0;
  char outstr[1000];
  time_t lasttime = 0;
  
  while ((nread = getline(&line, &len, stdin)) != -1) {
    count++;

    if ((mod == 1) || ((count % mod) == 1)) {
      fwrite(line, nread, 1, stdout);
    }

    int printprogress = 0;

    if (progress > 0) {
      if ((progress == 1) || ((count % progress) == 1)) {
	printprogress = 1;
      }
    }
    if (fewsecs > 0) {
      if ((time(NULL) - lasttime) >= fewsecs) {
	printprogress = 1;
      }
    }

    if (printprogress) {
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
      lasttime = time(NULL);
    }
  }
  
  free(line);
  exit(EXIT_SUCCESS);
}
