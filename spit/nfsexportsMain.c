#include "nfsexports.h"

#include <stdlib.h>

int main(void) {
  nfsRangeExports *n = nfsExportsInit();

  jsonValue* json = nfsExportsJSON(n);
  printf("%s\n", jsonValueDump(json));
  
  nfsExportsFree(&n);
  free(n);

  return 0;
}
