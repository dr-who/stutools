#ifndef __KEYVALUE_H
#define __KEYVALUE_H

#include <assert.h>
#include <malloc.h>

typedef struct {
  char *key;
  int type;
  union {
    char *value;
    long longvalue;
  };
} keyvaluePair;

typedef struct {
  size_t num;
  keyvaluePair *pairs;
  size_t checksum;

  size_t byteLen; //includes \0s
} keyvalueType;

keyvalueType *keyvalueInit();
keyvalueType *keyvalueInitFromString(char *parse);

size_t keyvalueSetString(keyvalueType *kv, const char *key, char *value);
size_t keyvalueSetLong(keyvalueType *kv, const char *key, long value);

void keyvalueAddUUID(keyvalueType *kv);

size_t keyvalueAddString(keyvalueType *kv, const char *key, char *value);

size_t keyvalueDeleteWithKey(keyvalueType *kv, char *key);

int keyvalueFindKey(keyvalueType *kv, const char *key);
int keyvalueGetType(keyvalueType *kv, const char *key);
long keyvalueGetLong(keyvalueType *kv, const char *key);
char *keyvalueGetString(keyvalueType *kv, const char *key);

char *keyvalueDumpAsString(keyvalueType *kv);
char *keyvalueDumpAsJSON(keyvalueType *kv);
void keyvalueFree(keyvalueType *kv);

void keyvalueDumpAtStartRAM(keyvalueType *kv, volatile void *ram);
void keyvalueDumpAtStartFD(keyvalueType *kv, int fd);

size_t keyvalueChecksum(keyvalueType *kv);
void keyvalueSort(keyvalueType *kv);

#endif

