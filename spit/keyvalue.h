#ifndef __KEYVALUE_H
#define __KEYVALUE_H

#include <assert.h>
#include <malloc.h>

typedef struct {
  char *key;
  char *value;
} keyvaluePair;

typedef struct {
  size_t num;
  keyvaluePair *pairs;

  size_t byteLen; //includes \0s
} keyvalueType;

keyvalueType *keyvalueInit() {
  keyvalueType *p = calloc(1, sizeof(keyvalueType)); assert(p);
  return p;
}

size_t keyvalueAdd(keyvalueType *kv, char *key, char *value);

size_t keyvalueDeleteWithKey(keyvalueType *kv, char *key);

char *keyvalueFindValueWithKey(keyvalueType *kv, char *key);

char *keyvalueAsString(keyvalueType *kv);

void keyvalueFree(keyvalueType *kv);

#endif

