#include "nfsexports.h"

#include <stdlib.h>

int main(void) {
  nfsRangeExports *n = nfsExportsInit();

  nfsExportsJSON(n);

  nfsExportsFree(&n);
  free(n);

  return 0;
}
