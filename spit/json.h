#ifndef __JSON_H
#define __JSON_H

#include <assert.h>
#include <malloc.h>


enum elType {J_OBJECT, J_ARRAY, J_STRING, J_NUMBER, J_TRUE, J_FALSE, J_NULL};

typedef struct _jsonValue jsonValue;


// [value, value, value]
typedef struct {
  size_t num;
  jsonValue **value;
} jsonArray;

// { key: value}
typedef struct {
  size_t num;
  char **key;
  jsonValue **value;
} jsonObject;


typedef struct _jsonValue {
  enum elType type;
  union {
    jsonObject *object; // { k: value}
    jsonArray *array;   // [ value, value, value ]
    char *string;       //
    long number;       //
    int val_true;
    int val_false;
    int val_null;
  };
} jsonValue;


// JSON elements is a sequency of jsonElement types
typedef struct {
  size_t num;
  jsonValue **value;
} jsonElements;


jsonElements *jsonInit(void);
char * jsonElementsDump(jsonElements *j, int pretty);
char * jsonValueDump(jsonValue *j, int pretty);

jsonValue *jsonValueNumber(long num);
jsonValue *jsonValueString(char *str);
jsonValue *jsonValueArray(void);
jsonValue *jsonValueObject(void);

void jsonObjectAdd(jsonValue *j, char *key, jsonValue *value);
void jsonArrayAdd(jsonValue *j, jsonValue *value);
void jsonElementAdd(jsonElements *j, jsonValue *value);


#endif

