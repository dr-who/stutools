#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mapVoid.h"
#include "numList.h"
#include "mapNumList.h"

void mapNumListInit(mapNumListType *h) {
    h->n = 0;
    h->map = NULL;
    h->key = NULL;
}

numListType *mapNumListAdd(mapNumListType *h, const char *name) {
    for (size_t i = 0; i < h->n; i++) {
        if (strcmp(h->key[i], name) == 0) {
            fprintf(stderr, "*info* found '%s'\n", name);
            return &h->map[i];
        } else {
            fprintf(stderr, "not %s\n", h->key[i]);
        }
    }
    // not found
    fprintf(stderr, "*info* add '%s'\n", name);
    h->n++;
    h->key = realloc(h->key, (h->n) * (sizeof(char *)));
    h->key[h->n - 1] = strdup(name);

    h->map = realloc(h->map, (h->n) * (sizeof(numListType)));
    nlInit(&h->map[h->n - 1], 100000);

    return &h->map[h->n - 1];
}

/*
int main(int argc, char *argv[]) {
  mapVoidType m;
  mapVoidInit(&m);

  numListType *n = (numListType*)mapVoidAdd(&m, "key");
  fprintf(stderr,"%p\n", n);

  numListType *n2 = (numListType*)mapVoidAdd(&m, "key");
  fprintf(stderr,"%p\n", n2);

  nlInit(n, 100);
  return 0;
}
*/
