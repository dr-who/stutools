#include <assert.h>
#include <malloc.h>
#include <string.h>

#include "dataset.h"

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;

  datasetType *d = datasetInit();


  datasetInitHeader(d, "l:country n:age\t n:amount \tn:items p:class\t\n");
    
  datasetAddDataString(d, "? 1 3 4 5\r\n3 4 2 3 ?\nbungy 3 4 5 sisd\nbongo,3,45,5,99\nok 4 5 6 98\n");

  datasetDumpJSON(d);

  datasetStats(d);


  datasetFree(&d);
  assert(d==NULL);

  return 0;
}
