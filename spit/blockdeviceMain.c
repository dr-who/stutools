#include "blockdevice.h"
#include "json.h"

int main(int argc, char *argv[]) {

  jsonValue *j = bdScan();

  char *s = jsonValueDump(j);

  printf("%s\n", s);
  free(s);
  
  return 0;
}
