
#include "keyvalue.h"
#include "string.h"


int main() {
  keyvalueType *kv = keyvalueInit();

  keyvalueSetString(kv, "stu", "nic");
  keyvalueSetString(kv, "stu2", "stu");
  keyvalueSetString(kv, "stu2", "sni");
  keyvalueSetLong(kv, "a", 8213);
  keyvalueSetLong(kv, "zz", -3489);
  keyvalueSetLong(kv, "stu", 42);
  keyvalueSetString(kv, "stu", "nic");
  keyvalueSetLong(kv, "bob", 12);
  keyvalueAddString(kv, "stu", "df");
  keyvalueSetLong(kv, "stu", 42);

  char * s = keyvalueDumpAsString(kv);
  fprintf(stderr, "%s\n", s);
  free(s);

  s = keyvalueDumpAsJSON(kv);
  printf("%s\n", s);
  free(s);

  keyvalueFree(kv);

  kv = keyvalueInitFromString("stu:nic bob;12 a;83 z:qq");
  char *sss = keyvalueDumpAsJSON(kv);
  fprintf(stderr,"%s", sss);
  keyvalueFree(kv);

  return 0;
}
