
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

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

    int tok1alpha = 1;
    if (tok1) {
      for (size_t q=0 ; q < strlen(tok1); q++) {
	if (isalpha(tok1[q]) == 0) tok1alpha = 0;
      }
    }
    int tok2alpha = 1;
    if (tok2) {
      for (size_t q=0 ; q < strlen(tok2); q++) {
	if (isalpha(tok2[q]) == 0) tok2alpha = 0;
      }
    }

    if (tok2) { // if we have two valid values
      lineno++;
      if (lineno == 1) {
	if (tok1alpha || tok2alpha) { // header
	  nlSetLabel(&first, tok1);
	  nlSetLabel(&second, tok2);
	} else {
	  nlSetLabel(&first, "column1");
	  nlSetLabel(&second, "column2");
	  nlAdd(&first, atof(tok1));
	  nlAdd(&second, atof(tok2));
	}
      } else {
	nlAdd(&first, atof(tok1));
	nlAdd(&second, atof(tok2));
      }
    } else {
      fprintf(stderr,"skipping line without 2 values between a tab\n");
    }
  }

  if (nlN(&first) >= 1) {
    
    fprintf(stdout,"'%10s', mean: %.4lf, N %zd, SD %.4lf, SEM %.4lf, median %.4lf, 99.999%% %.4lf\n", nlLabel(&first), nlMean(&first), nlN(&first), nlSD(&first), nlSEM(&first), nlMedian(&first), nlSortedPos(&first, 0.99999));
    fprintf(stdout,"'%10s', mean: %.4lf, N %zd, SD %.4lf, SEM %.4lf, median %.4lf, 99.999%% %.4lf\n", nlLabel(&second),  nlMean(&second),nlN(&second), nlSD(&second), nlSEM(&second), nlMedian(&second), nlSortedPos(&second, 0.99999));
    
    double r = 0;
    nlCorrelation(&first, &second, &r);
    fprintf(stdout,"r = %.4lf\n", r);

    double unsd = 0;
    nlUnbiasedSD(&first, &second, r, &unsd);
  }

  free(line);
  exit(EXIT_SUCCESS);
}
