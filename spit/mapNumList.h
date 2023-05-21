#ifndef _MAPNUMLIST_H
#define _MAPNUMLIST_H

#include "numList.h"

typedef struct {
    size_t n;
    char **key;
    numListType *map;
} mapNumListType;

void mapNumListInit(mapNumListType *h);

numListType *mapNumListAdd(mapNumListType *h, const char *name);


#endif

