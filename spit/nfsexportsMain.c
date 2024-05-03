#include "nfsexports.h"

#include <stdlib.h>

int main(void) {
  nfsRangeExports *n = nfsExportsInit();

  jsonValue* json = nfsExportsJSON(n);
  printf("%s\n", jsonValueDump(json, 1));
  
  nfsExportsFree(&n);
  free(n);

  return 0;
}
