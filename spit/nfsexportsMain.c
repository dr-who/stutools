#include "nfsexports.h"

#include <stdlib.h>

int main(void) {
  nfsRangeExports *n = nfsExportsInit();

  nfsExportsJSON(n);
  nfsExportsKV(n);

  nfsExportsFree(&n);
  free(n);

  return 0;
}
