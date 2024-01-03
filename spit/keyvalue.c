
#include "keyvalue.h"
#include "string.h"

keyvalueType *keyvalueInit() {
  keyvalueType *p = calloc(1, sizeof(keyvalueType)); assert(p);
  return p;
}


size_t keyvalueAdd(keyvalueType *kv, char *key, char *value) {
  int index = -1;
  for (size_t i = 0; i < kv->num; i++) {
    if (strcmp(key, kv->pairs[i].key)==0) {
      index = i;
      break;
    }
  }

  if (index >= 0) {
    //found
    free(kv->pairs[index].key);
    free(kv->pairs[index].value);
  } else {
    index = kv->num;
    kv->num++;
    // add
    kv->pairs = realloc(kv->pairs, kv->num * sizeof(keyvaluePair));
  }
  kv->pairs[index].key = strdup(key);
  kv->pairs[index].value = strdup(value);

  kv->byteLen += (strlen(key)+2);
  kv->byteLen += (strlen(value)+2);
  return index;
}



void keyvalueSort(keyvalueType *kv) {
  for (size_t i = 0; i < kv->num - 1; i++) {
    for (size_t j = i+1; j < kv->num; j++) {
      int t = strcmp(kv->pairs[i].key, kv->pairs[j].key);
      if (t > 0) {
	keyvaluePair tmp = kv->pairs[i];
	kv->pairs[i] = kv->pairs[j];
	kv->pairs[j] = tmp;
	//swap
      }
    }
  }
}


char *keyvalueAsString(keyvalueType *kv) {
  keyvalueSort(kv);
  char *s = calloc(1, kv->byteLen);
  for (size_t i = 0; i < kv->num; i++) {
    if (i > 0) {
      strcat(s, " ");
    }
    strcat(s, kv->pairs[i].key);
    strcat(s, ":");
    strcat(s, kv->pairs[i].value);
  }
  return s;
}

void keyvalueFree(keyvalueType *kv) {
  for (size_t i = 0 ; i < kv->num; i++) {
    free(kv->pairs[i].key);
    free(kv->pairs[i].value);
  }
  free(kv->pairs);
  free(kv);
}

