
#include "keyvalue.h"
#include "string.h"

keyvalueType *keyvalueInit() {
  keyvalueType *p = calloc(1, sizeof(keyvalueType)); assert(p);
  return p;
}


int keyvalueFindKey(keyvalueType *kv, const char *key) {
  for (size_t i = 0; i < kv->num; i++) {
    if (strcmp(key, kv->pairs[i].key)==0) {
      return i;
      break;
    }
  }
  return -1;
}


size_t keyvalueAddString(keyvalueType *kv, const char *key, char *value) {
  int index = keyvalueFindKey(kv, key);

  if (index >= 0) {
    //found
    if (kv->pairs[index].type == 0) free(kv->pairs[index].key);
    free(kv->pairs[index].value);
  } else {
    index = kv->num;
    kv->num++;
    // add
    kv->pairs = realloc(kv->pairs, kv->num * sizeof(keyvaluePair));
  }
  kv->pairs[index].key = strdup(key);
  kv->pairs[index].value = strdup(value);
  kv->pairs[index].type = 2;

  kv->byteLen += (strlen(key)+2);
  kv->byteLen += (strlen(value)+2);
  return index;
}

size_t keyvalueSetString(keyvalueType *kv, const char *key, char *value) {
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
  kv->pairs[index].type = 0;

  kv->byteLen += (strlen(key)+2);
  kv->byteLen += (strlen(value)+2);
  return index;
}


size_t keyvalueSetLong(keyvalueType *kv, const char *key, long value) {
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
    if (kv->pairs[index].type == 0) {
      // string
      free(kv->pairs[index].value);
    }
  } else {
    index = kv->num;
    kv->num++;
    // add
    kv->pairs = realloc(kv->pairs, kv->num * sizeof(keyvaluePair));
  }
  kv->pairs[index].key = strdup(key);
  kv->pairs[index].longvalue = value;
  kv->pairs[index].type = 1; // 1 long

  kv->byteLen += (strlen(key)+2);
  kv->byteLen += (20+2);
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


int keyvalueGetType(keyvalueType *kv, const char *key) {
  int index = keyvalueFindKey(kv, key);
  if (index >= 0) {
    return kv->pairs[index].type;
  } else {
    return -1;
  }
}

long keyvalueGetLong(keyvalueType *kv, const char *key) {
  int index = keyvalueFindKey(kv, key);
  assert(kv->pairs[index].type == 1);
  if (index >= 0) {
    return kv->pairs[index].longvalue;
  } else {
    return 0;
  }
}

char *keyvalueGetValue(keyvalueType *kv, const char *key) {
  int index = keyvalueFindKey(kv, key);
  assert(kv->pairs[index].type == 0);
  if (index >= 0) {
    return kv->pairs[index].value;
  } else {
    return NULL;
  }
}

  
char *keyvalueDumpAsString(keyvalueType *kv) {
  keyvalueSort(kv);
  char *s = calloc(1, kv->byteLen);
  for (size_t i = 0; i < kv->num; i++) {
    if (i > 0) {
      strcat(s, " ");
    }
    strcat(s, kv->pairs[i].key);
    strcat(s, ":");
    if (kv->pairs[i].type == 0 || kv->pairs[i].type==2) { // 0 == string
      strcat(s, kv->pairs[i].value);
    } else {
      char str[100];
      sprintf(str, "%ld", kv->pairs[i].longvalue);
      strcat(s, str);
    }
  }
  return s;
}

char *keyvalueDumpAsJSON(keyvalueType *kv) {
  keyvalueSort(kv);
  char *s = calloc(1, 2*kv->byteLen);

  strcat(s, "{\n");
  for (size_t i = 0; i < kv->num; i++) {
    strcat(s, "\"");
    strcat(s, kv->pairs[i].key);
    strcat(s, "\"");
    strcat(s, ": ");
    if (kv->pairs[i].type == 0) { // single string
      strcat(s, "\"");
      strcat(s, kv->pairs[i].value);
      strcat(s, "\"");
    } else if (kv->pairs[i].type == 2) { //list
      strcat(s, "[ { \"device\": \"");
      strcat(s, kv->pairs[i].value);
      strcat(s, "\" } ]");
    } else {
      char str[100];
      sprintf(str, "%ld", kv->pairs[i].longvalue);
      strcat(s, str);
    }
    strcat(s, "\n");
    
    if (i < kv->num-1) {
      strcat(s, ",");
    }
  }
  
  strcat(s, "}\n");
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

