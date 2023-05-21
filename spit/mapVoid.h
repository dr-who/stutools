#ifndef _MAPVOID_H
#define _MAPVOID_H

typedef struct {
    size_t n;
    char **key;
    void **map;
} mapVoidType;

void mapVoidInit(mapVoidType *h);

void *mapVoidFind(mapVoidType *h, const char *name);

void *mapVoidAdd(mapVoidType *h, const char *name, void *p);


#endif

