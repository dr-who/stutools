
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "numList.h"


int main() {

  fprintf(stderr,"*info* stat - reads one columns of numbers\n");
  
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
      nlAdd(&first, atof(tok1));
    } else {
      fprintf(stderr,"skipping line without a good value\n");
    }
  }

  if (nlN(&first) >= 1) {
    fprintf(stdout,"mean: %.4lf Q25%%: %.4lf Median: %.4lf Q75%%: %.4lf N:%zd SD: %.4lf SEM: %.4lf\n", nlMean(&first), nlSortedPos(&first, 0.25), nlMedian(&first), nlSortedPos(&first, 0.75),  nlN(&first), nlSD(&first), nlSEM(&first));
  }
  fprintf(stderr,"*info* stat - N: %zd Mean: %.4lf\n", nlN(&first), nlMean(&first));

  free(line);
  exit(EXIT_SUCCESS);
}
