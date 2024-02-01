#define _POSIX_C_SOURCE  200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mapVoid.h"
#include "numList.h"

#include <assert.h>

void mapVoidInit(mapVoidType *h) {
    h->n = 0;
    h->map = NULL;
    h->key = NULL;
}

void *mapVoidFind(mapVoidType *h, const char *name) {
    for (size_t i = 0; i < h->n; i++) {
        if (strcmp(h->key[i], name) == 0) {
            return h->map[i];
        }
    }
    return NULL;
}

void *mapVoidAdd(mapVoidType *h, const char *name, void *p) {
    h->n++;
    h->key = realloc(h->key, (h->n) * (sizeof(char *)));
    assert(h->key);
    h->key[h->n - 1] = strdup(name);

    h->map = realloc(h->map, (h->n) * (sizeof(void *)));
    assert(h->map);
    h->map[h->n - 1] = p;
    return p;
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
