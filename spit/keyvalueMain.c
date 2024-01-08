
#include "keyvalue.h"
#include "string.h"


int main() {
  keyvalueType *kv = keyvalueInit();

  keyvalueSetString(kv, "stu", "nic");
  keyvalueSetString(kv, "stu2", "stu");
  keyvalueSetString(kv, "stu2", "sni");
  keyvalueSetLong(kv, "a", 8213);
  keyvalueSetLong(kv, "zz", -3489);

  char * s = keyvalueDumpAsString(kv);
  fprintf(stderr, "%s\n", s);
  free(s);

  s = keyvalueDumpAsJSON(kv);
  printf("%s\n", s);
  free(s);

  keyvalueFree(kv);

  return 0;
}
