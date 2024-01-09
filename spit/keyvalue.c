#include <stdlib.h>

#include "keyvalue.h"
#include "string.h"


keyvalueType *keyvalueInit() {
  keyvalueType *p = calloc(1, sizeof(keyvalueType)); assert(p);
  return p;
}


void keyvalueParsePair(keyvalueType *kv, char *pp) {
  char *ch = strchr(pp, ':');
  if (ch) {
    *ch = 0;
    char *key = pp;
    char *value = ch+1;
    
    keyvalueSetString(kv, strdup(key), strdup(value));
  } else {
    ch = strchr(pp, ';'); // longs
    if (ch) {
      *ch = 0;
      char *key = pp;
      char *value = ch+1;
      keyvalueSetLong(kv, strdup(key), atol(value));
    } else {
      fprintf(stderr,"pair with '%s' has no value\n", pp);
    }
  }
}
    
  

keyvalueType *keyvalueInitFromString(char *par) {
  keyvalueType *p = keyvalueInit();
  char *parse = strdup(par);
  char *tok = strtok(parse, " ");
  if (tok) {
    keyvalueParsePair(p, tok);
    
    while ((tok = strtok(NULL, " "))) {
      keyvalueParsePair(p, tok);
    }
  }
  free(parse);
  //  fprintf(stderr,"bytes to use %zd\n", p->byteLen);
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
    free(kv->pairs[index].key);
   if (kv->pairs[index].type == 0 || kv->pairs[index].type==2) free(kv->pairs[index].value);
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
    if (kv->pairs[index].type == 0 || kv->pairs[index].type==2) free(kv->pairs[index].value);
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
    if (kv->pairs[index].type == 0 || kv->pairs[index].type == 2) {
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

char *keyvalueGetString(keyvalueType *kv, const char *key) {
  int index = keyvalueFindKey(kv, key);
  assert(kv->pairs[index].type != 1);
  if (index >= 0) {
    return kv->pairs[index].value;
  } else {
    return NULL;
  }
}

  
char *keyvalueDumpAsString(keyvalueType *kv) {
  keyvalueSort(kv);
  int maxbytes = 3*kv->byteLen;
  
  char *s = calloc(maxbytes, 1); assert(s);
  for (size_t i = 0; i < kv->num; i++) {
    if (i > 0) {
      strncat(s, " ", maxbytes);
    }
    strncat(s, kv->pairs[i].key, maxbytes);
    if (kv->pairs[i].type == 0 || kv->pairs[i].type==2) { // 0 == string
      strncat(s, ":", maxbytes);
      strncat(s, kv->pairs[i].value, maxbytes);
    } else {
      char str[1000];
      sprintf(str, "%ld", kv->pairs[i].longvalue);
      strncat(s, ";", maxbytes);
      strncat(s, str, maxbytes);
    }
  }
  //  strncat(s, "\n", maxbytes); // end with a 0?
  return s;
}

char *keyvalueDumpAsJSON(keyvalueType *kv) {
  keyvalueSort(kv);
  char *s = calloc(3*kv->byteLen, 1);
  size_t level = 0;

  strcat(s, "{\n");
  for (size_t i = 0; i < kv->num; i++) {
    // tabs
    level++; for (size_t l = 0; l < level; l++) strcat(s, "\t");
    strcat(s, "\"");
    strcat(s, kv->pairs[i].key);
    strcat(s, "\"");
    strcat(s, ": ");
    if (kv->pairs[i].type == 0) { // single string
      strcat(s, "\"");
      strcat(s, kv->pairs[i].value);
      strcat(s, "\"");
    } else if (kv->pairs[i].type == 2) { //list
      //tabs
      level++; for (size_t l = 0; l < level; l++) strcat(s, "\t");
      strcat(s, "[ { \"device\": \"");
      strcat(s, kv->pairs[i].value);
      strcat(s, "\" } ]");
      level--;
    } else {
      char str[1000];
      sprintf(str, "%ld", kv->pairs[i].longvalue);
      strcat(s, str);
    }
    
    if (i < kv->num-1) {
      strcat(s, ",");
    }
    strcat(s, "\n");
    level--;
  }
  
  strcat(s, "}\n");
  return s;
}

void keyvalueFree(keyvalueType *kv) {
  for (size_t i = 0 ; i < kv->num; i++) {
    free(kv->pairs[i].key);
    if (kv->pairs[i].type == 0 || kv->pairs[i].type==2) free(kv->pairs[i].value);
  }
  free(kv->pairs);
  free(kv);
}

