
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "numList.h"


int main() {

  fprintf(stderr,"*info* pair - reads two tab separated columns of numbers. With a header line\n");
  
  numListType first, second;
  nlInit(&first, 1999);
  nlInit(&second, 1999);
  
  // first line is header use tabs
  
  FILE *stream = stdin;
  char *line = NULL, *saveptr = NULL;
  size_t len = 0;
  ssize_t nread, lineno = 0;
  
  while ((nread = getline(&line, &len, stream)) != -1) {
    if (strchr(line, '\t') == NULL) {
      fprintf(stderr,"skipping line with no tab: %s", line);
      continue;
    }

    char *tok1 = strtok_r(line, "\t\r\n", &saveptr);
    char *tok2 = strtok_r(saveptr, "\t\n\r", &saveptr);

    if (tok2) { // if we have two valid values
      lineno++;
      if (lineno == 1) { // header
	nlSetLabel(&first, tok1);
	nlSetLabel(&second, tok2);
      } else {
	nlAdd(&first, atof(tok1));
	nlAdd(&second, atof(tok2));
      }
    } else {
      fprintf(stderr,"skipping line without 2 values between a tab\n");
    }
  }

  if (nlN(&first) >= 2) {
    
    fprintf(stdout,"'%10s', mean: %.4lf, N %zd, SD %.4lf, SEM %.4lf\n", nlLabel(&first), nlMean(&first), nlN(&first), nlSD(&first), nlSEM(&first));
    fprintf(stdout,"'%10s', mean: %.4lf, N %zd, SD %.4lf, SEM %.4lf\n", nlLabel(&second),  nlMean(&second),nlN(&second), nlSD(&second), nlSEM(&second));
    
    double r = 0;
    nlCorrelation(&first, &second, &r);
    fprintf(stdout,"r = %.4lf\n", r);

    double unsd = 0;
    nlUnbiasedSD(&first, &second, r, &unsd);
  }

  free(line);
  exit(EXIT_SUCCESS);
}
