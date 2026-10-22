
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "numList.h"


int main(int argc, char *argv[]) {

    if (argv)
        assert(argv);

    numListType first;
    nlInit(&first, 10000000); // 10M values

    // first line is header use tabs

    FILE *stream = stdin;
    char *line = NULL, *saveptr = NULL;
    size_t len = 0;
    ssize_t nread, lineno = 0;

    while ((nread = getline(&line, &len, stream)) != -1) {
        if (strchr(line, '\t') != NULL) {
            fprintf(stderr, "skipping line %zd with a tab: %s", lineno, line);
            continue;
        }

        char *tok1 = strtok_r(line, "\t\r\n", &saveptr);

        if (tok1) {
            char *endptr = NULL;
            double result = strtod(tok1, &endptr);
            if (endptr != tok1 + strlen(tok1)) {
                fprintf(stderr, "skipping line '%s'\n", tok1);
            } else {
                nlAdd(&first, result);
            }
        } else {
            fprintf(stderr, "skipping line\n");
        }

    }


    if (argc < 2) {
        if (nlN(&first) >= 1) {
    	  double mean = NAN, max = NAN;
	  mean = nlMean(&first);
	  max = nlMax(&first);
	  
            fprintf(stdout,
                    "mean: \t%.3lf\t(%.0lf bits)\nmin: \t%.3lf\nQ25%%: \t%.3lf\nMed: \t%.3lf\nQ75%%: \t%.3lf\nmax: \t%.3lf\t(%.0lf bits)\nN:   \t%zd\nSD: \t%.3lf\nSEM: \t%.3lf\n",
                    mean, ceil(log(mean+1)/log(2)), nlMin(&first), nlSortedPos(&first, 0.25), nlMedian(&first),
                    nlSortedPos(&first, 0.75), max, ceil(log(max+1)/log(2)), nlN(&first), nlSD(&first), nlSEM(&first));
            fprintf(stdout, "99.9%%  \t%.3lf\n", nlSortedPos(&first, 0.999));
            fprintf(stdout, "99.99%% \t%.3lf\n", nlSortedPos(&first, 0.9999));
            fprintf(stdout, "99.999%%\t%.3lf\n", nlSortedPos(&first, 0.99999));
        }
        fprintf(stderr, "*info* stat - N: %zd Mean: %.4lf\n", nlN(&first), nlMean(&first));
    } else {
        for (int n = 1; n < argc; n++) {
	  if (n > 1) fprintf(stdout,"\t");
	  if (strcmp(argv[n], "mean") == 0) fprintf(stdout, "%lf", nlMean(&first));
	  else if (strcmp(argv[n], "median") == 0) fprintf(stdout, "%lf", nlMedian(&first));
	  else if (strcmp(argv[n], "min") == 0) fprintf(stdout, "%lf", nlMin(&first));
	  else if (strcmp(argv[n], "max") == 0) fprintf(stdout, "%lf", nlMax(&first));
	  else if (strcmp(argv[n], "n") == 0) fprintf(stdout, "%zd", nlN(&first));
	  else if (strcmp(argv[n], "sd") == 0) fprintf(stdout, "%lf", nlSD(&first));
	  else if (strcmp(argv[n], "sem") == 0) fprintf(stdout, "%lf", nlSEM(&first));
	  else if (strcmp(argv[n], "q25") == 0) fprintf(stdout, "%lf", nlSortedPos(&first, 0.25));
	  else if (strcmp(argv[n], "q75") == 0) fprintf(stdout, "%lf", nlSortedPos(&first, 0.75));
	  else if (argv[n][0] == 'q') {
            float qq = atof(argv[n] + 1) / 100;
            if (qq < 0) qq = -1;
            if (qq >= 1) qq = -1;
            fprintf(stdout, "%lf", nlSortedPos(&first, qq));
	  }
	  else {
            fprintf(stderr, "*error* unknown command '%s'\n", argv[n]);
            exit(1);
	  }
	}
	fprintf(stdout,"\n");
    }

    free(line);
    exit(EXIT_SUCCESS);
}
