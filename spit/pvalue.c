
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

#include "numList.h"

int main(int argc, char *argv[]) {
  if (argc < 2) {
    fprintf(stderr,"usage: pvalue <df>\n");
    exit(1);
  }
  int df = atoi(argv[1]);
  double p = loadTTable(df, 2, 0.05);
  fprintf(stdout, "two tailed, critical value p = %.4lf\n", p);

  p = loadTTable(df, 1, 0.05);
  fprintf(stdout, "one tailed, critical value p = %.4lf\n", p);

  return 0;
}
  
