
#include "keyvalue.h"
#include "string.h"


int main() {
  keyvalueType *kv = keyvalueInit();

  keyvalueAdd(kv, "stu", "nic");
  keyvalueAdd(kv, "stu2", "stu");
  keyvalueAdd(kv, "stu2", "sni");
  keyvalueAdd(kv, "a", "b");
  keyvalueAdd(kv, "zz", "b");

  char * s = keyvalueAsString(kv);
  printf("%s\n", s);
  free(s);

  keyvalueFree(kv);

  return 0;
}
