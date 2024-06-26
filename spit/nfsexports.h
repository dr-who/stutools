#ifndef _NFSEXPORTS_H
#define _NFSEXPORTS_H

#include "json.h"

typedef struct {
  char *prefix;
  int start;
  int end;
  int missing;
  long minSizeGB;
  long maxSizeGB;
  long sumSizeGB;
  long sumFreeGB;
  char *type;
} nfsRangeType;


typedef struct {
  int num;
  nfsRangeType *exports;
} nfsRangeExports;


nfsRangeExports *nfsExportsInit(void); // init and scan
void nfsExportsFree(nfsRangeExports **n); // free sub and pointer, change to NULL

char *nfsExportsKV(nfsRangeExports *n);   // return nice KV format

void nfsPrefixUpdate(nfsRangeExports *n, const char *prefix, int suffix, const char *scope);

jsonValue * nfsExportsJSON(nfsRangeExports *n);
  
#endif
