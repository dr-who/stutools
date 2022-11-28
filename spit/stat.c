
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "numList.h"


int main(int argc, char *argv[]) {

  if (argv)
    assert(argv);
  
  numListType first;
  nlInit(&first, 1000000);
  
  // first line is header use tabs
  
  FILE *stream = stdin;
  char *line = NULL, *saveptr = NULL;
  size_t len = 0;
  ssize_t nread, lineno = 0;
  
  while ((nread = getline(&line, &len, stream)) != -1) {
    if (strchr(line, '\t') != NULL) {
      fprintf(stderr,"skipping line %zd with a tab: %s", lineno, line);
      continue;
    }

    char *tok1 = strtok_r(line, "\t\r\n", &saveptr);

    if (tok1) {
      char *endptr = NULL;
      double result = strtod(tok1, &endptr);
      if (endptr != tok1 + strlen(tok1)) {
	fprintf(stderr,"skipping line '%s'\n", tok1);
      } else {      
	nlAdd(&first, result);
      }
    } else {
      fprintf(stderr,"skipping line\n");
    }

  }


  if (argc < 2) {
    if (nlN(&first) >= 1) {
      fprintf(stdout,"mean: %.4lf min: %.4lf Q25%%: %.4lf Median: %.4lf Q75%%: %.4lf max: %.4lf N:%zd SD: %.4lf SEM: %.4lf\n", nlMean(&first), nlMin(&first), nlSortedPos(&first, 0.25), nlMedian(&first), nlSortedPos(&first, 0.75), nlMax(&first),  nlN(&first), nlSD(&first), nlSEM(&first));
    }
    fprintf(stderr,"*info* stat - N: %zd Mean: %.4lf\n", nlN(&first), nlMean(&first));
  } else {
    if (strcmp(argv[1],"mean")==0) fprintf(stdout, "%lf\n", nlMean(&first));
    else if (strcmp(argv[1],"median")==0) fprintf(stdout, "%lf\n", nlMedian(&first));
    else if (strcmp(argv[1],"min")==0) fprintf(stdout, "%lf\n", nlMin(&first));
    else if (strcmp(argv[1],"max")==0) fprintf(stdout, "%lf\n", nlMax(&first));
    else if (strcmp(argv[1],"n")==0) fprintf(stdout, "%zd\n", nlN(&first));
    else if (strcmp(argv[1],"sd")==0) fprintf(stdout, "%lf\n", nlSD(&first));
    else if (strcmp(argv[1],"sem")==0) fprintf(stdout, "%lf\n", nlSEM(&first));
    else if (strcmp(argv[1],"q25")==0) fprintf(stdout, "%lf\n", nlSortedPos(&first, 0.25));
    else if (strcmp(argv[1],"q75")==0) fprintf(stdout, "%lf\n", nlSortedPos(&first, 0.75));
    else if (argv[1][0] == 'q') {float qq = atof(argv[1]+1)/100; if (qq<0) qq=-1; if (qq>=1) qq=-1; fprintf(stdout, "%lf\n", nlSortedPos(&first, qq));}
    else {
      fprintf(stderr,"*error* unknown command '%s'\n", argv[1]);
      exit(1);
    }
  }

  free(line);
  exit(EXIT_SUCCESS);
}
