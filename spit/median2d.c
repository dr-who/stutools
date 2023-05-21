#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <getopt.h>

#include "numList.h"


typedef struct {
    numListType list;
    double key;
} pairType;

int find(double key, pairType *numbers, size_t numcount) {
    if (numbers) {
        for (size_t i = 0; i < numcount; i++) {
            if (key == numbers[i].key) {
                return i;
            }
        }
    }
    return -1; // -1 is not found
}

pairType *addKey(double key, pairType *numbers, size_t *numcount) {
    pairType *p = numbers;

    size_t s = *numcount;

    p = realloc(p, sizeof(pairType) * (s + 1));
    nlInit(&p[s].list, 1000);
    p[s].key = key;

    *numcount = (*numcount) + 1;
    return p;
}

pairType *addValues(double key, pairType *numbers, size_t *numcount, double val) {
    int f = find(key, numbers, *numcount);
    if (f == -1) {
        numbers = addKey(key, numbers, numcount);
        f = *numcount - 1;
        // added
    }
    nlAdd(&numbers[f].list, val);

    return numbers;
}

void usage() {
    fprintf(stdout, "Usage:\n  median2d [-m modulo]\n");
    fprintf(stdout, "\nDescription:\n");
    fprintf(stdout, "  Takes pairs of values (key value) per line. Groups the values per\n");
    fprintf(stdout, "  key and displays the value, median, N, SD and a list of all values.\n");
    fprintf(stdout, "  Optionally the -m module operation can be used, for say 360 degrees.\n");
}


int main(int argc, char *argv[]) {

    int opt;
    size_t modulo = 0;

    const char *getoptstring = "m:h";
    while ((opt = getopt(argc, argv, getoptstring)) != -1) {
        switch (opt) {
            case 'm':
                modulo = atoi(optarg);
                fprintf(stderr, "*info* performing modulo %zd\n", modulo);
                break;
            case 'h':
                usage();
                exit(1);
            default:
                break;
        }
    }


    pairType *numbers = NULL;
    size_t numcount = 0;

    char *line = NULL;
    size_t len = 0;
    ssize_t read = 0;

    while ((read = getline(&line, &len, stdin)) != -1) {
        double k;
        double v;
        int s = sscanf(line, "%lf %lf\n", &k, &v);
        if (s == 2) {
            if (modulo) {
                numbers = addValues(k, numbers, &numcount, fmod(v, modulo));
            } else {
                numbers = addValues(k, numbers, &numcount, v);
            }
            //      fprintf(stderr,"adding key %lf, value %lf\n", k, v);
        } else {
            fprintf(stderr, "*error* expecting two numbers, the key and the value\n");
        }
    }
    free(line);


    for (size_t i = 0; i < numcount; i++) {
        double themax = nlSortedPos(&numbers[i].list, 1);
        const size_t N = nlN(&numbers[i].list);
        double median = 0, mean = 0;

        numListType *p = NULL;

        fprintf(stdout, "%8.0lf\t", numbers[i].key);

        if (modulo) { // if min lower half it's too close to zero, add

            themax = nlSortedPos(&numbers[i].list, 1);

            numListType better;
            nlInit(&better, 1000);
            for (size_t j = 0; j < N; j++) {
                const double d1 = fabs(numbers[i].list.values[j].value - themax);
                const double d2 = fabs(numbers[i].list.values[j].value + modulo - themax);

                if (d1 < d2) {
                    nlAdd(&better, numbers[i].list.values[j].value);
                } else {
                    //	  fprintf(stderr,"%lf %lf\n", d1,d2);
                    nlAdd(&better, modulo + numbers[i].list.values[j].value);
                }
            }
            themax = nlSortedPos(&better, 1);
            median = fmod(nlMedian(&better), modulo);
            mean = nlMean(&better);
            p = &better;
        } else {
            median = nlMedian(&numbers[i].list);
            if (modulo) median = fmod(median, modulo);
            p = &numbers[i].list;
            mean = nlMean(&numbers[i].list);
        }

        fprintf(stdout, "median %lf\tmode %lf\tmean %lf\tN %zd\tSD %lf\t|\t", median, nlMode(p, 360, 1), mean, N,
                nlSD(p));
        for (size_t j = 0; j < N; j++) {
            size_t count = 1;
            for (size_t j2 = j + 1; j2 < N; j2++) {
                if (p->values[j].value == p->values[j2].value) {
                    count++;
                }
            }
            fprintf(stdout, "\t%lf(%zd)", p->values[j].value, count);
            j += (count - 1);
        }
        fprintf(stdout, "\n");

    }
    return 0;
}
    
