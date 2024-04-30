#include "json.h"

#include <assert.h>
#include <string.h>


jsonElements *jsonInit(void) {
  jsonElements *j = calloc(1, sizeof(jsonElements)); assert(j);
  return j;
}

char * jsonValueDump(jsonValue *val) {
  int slen = 1000000;
  char *s = calloc(slen, 1);
  char *ret = s;

  
  if (val->type == J_NUMBER) s += sprintf(s, "%ld ", val->number);
  else if (val->type == J_STRING) s += sprintf(s, "\"%s\" ", val->string);
  else if (val->type == J_ARRAY) {
    s += sprintf(s, "[");
    for (size_t i = 0; i < val->array->num; i++) {
      if (i > 0)
	s += sprintf(s, ",");
      s = stpcpy(s, jsonValueDump(val->array->value[i]));
    }
    s += sprintf(s, "]");
  } else if (val->type == J_OBJECT) {
    s += sprintf(s, "{");
    for (size_t i = 0; i < val->object->num; i++) {
      if (i > 0)
	s += sprintf(s, ",");
      s += sprintf(s, "\"%s\": ", val->object->key[i]);
      s = stpcpy(s, jsonValueDump(val->object->value[i]));
    }
    s += sprintf(s, "}");
  } else if (val->type == J_TRUE) {
    s += sprintf(s, "true");
  } else if (val->type == J_FALSE) {
    s += sprintf(s, "false");
  } else if (val->type == J_NULL) {
    s += sprintf(s, "null");
  }
  return ret;
}



char * jsonElementsDump(jsonElements *j) {
  char *s = calloc(100000,1);
  char *ret = s;
  
  for (size_t i = 0; i < j->num; i++) {
    if (i > 0)
      s += sprintf(s, ",");
    s = stpcpy(s,jsonValueDump(j->value[i]));
  }

  return ret;
}


jsonValue *jsonValueNumber(long num) {
  jsonValue *j = calloc(1, sizeof(jsonValue));
  j->type = J_NUMBER;
  j->number = num;
  return j;
}

jsonValue *jsonValueString(char *str) {
  jsonValue *j = calloc(1, sizeof(jsonValue));
  j->type = J_STRING;
  j->string = str ? strdup(str) : strdup("");
  return j;
}

jsonValue *jsonValueArray(void) {
  jsonValue *j = calloc(1, sizeof(jsonValue));
  j->type = J_ARRAY;
  j->array = calloc(1, sizeof(jsonArray));
  return j;
}

jsonValue *jsonValueObject(void) {
  jsonValue *j = calloc(1, sizeof(jsonValue));
  j->type = J_OBJECT;
  j->object = calloc(1, sizeof(jsonObject));
  return j;
}

void jsonObjectAdd(jsonValue *j, char *key, jsonValue *value) {
  assert(j->type == J_OBJECT);
  jsonObject *object = j->object;
  object->num++;
  object->key = realloc(object->key, (object->num) * sizeof(char*)); assert(object->key);
  object->value = realloc(object->value, (object->num) * sizeof(jsonValue*)); assert(object->key);
  object->key[object->num-1] = strdup(key);
  object->value[object->num-1] = value;
}

void jsonArrayAdd(jsonValue *j, jsonValue *value) {
  jsonArray *array = j->array;
  assert(j->type == J_ARRAY);
  array->num++;
  array->value = realloc(array->value, (array->num) * sizeof(jsonValue*)); assert(array->value);
  array->value[array->num-1] = value;
}


void jsonElementAdd(jsonElements *j, jsonValue *value) {
  j->num++;
  j->value = realloc(j->value, (j->num) * sizeof(jsonValue*)); assert(j->value);
  j->value[j->num-1] = value;
}


