#include "nfsexports.h"
#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
#include <assert.h>

#include <sys/statvfs.h>


#define PATH_MAX 1024


nfsRangeExports *nfsExportsInit(void) {
  FILE *fp;
  int status;
  char path[PATH_MAX];
  
  fp = popen("/usr/sbin/exportfs -s| sort -R", "r");
  if (fp == NULL) {
    perror("can't run exportfs");
    return NULL;
  }

  nfsRangeExports *n = calloc(1, sizeof(nfsRangeExports));assert(n);
  
  while (fgets(path, PATH_MAX, fp) != NULL) {
    char *first = strtok(path, "-\r\n \t()");
    //        printf("%s - ", first);
    if (first) {
      char *second = strtok(NULL, "-\r\n \t()");
      //        printf("%s -- ", second);
      if (second) {
	char *third = strtok(NULL, "-\r\n \t()");
	//    printf("%s \n", third);
	if (third) {
	  nfsPrefixUpdate(n, first, atoi(second), third);
	}
      }
    }
  }
  
  status = pclose(fp);
  if (status == -1) {
    /* Error reported by pclose() */
  } else {
    /* Use macros described under wait() to inspect `status' in order
       to determine success/failure of command executed by popen() */
  }
  return n;
}



void nfsPrefixUpdate(nfsRangeExports *n, const char *prefix, int suffix, const char *scope) {
  (void)scope;
  if (suffix < 1) {
    fprintf(stderr,"*error* suffixes < 1 not supported ('%s %d %s')\n", prefix, suffix, scope);
    return;
  }
  if (n) {
    int found = -1;
    for (int i = 0; i < n->num; i++) {
      if (n->exports[i].prefix && (strcmp(n->exports[i].prefix, prefix)==0)) {
	found = i;
      }
    }
    if (found == -1) { // not found add
      n->exports = realloc(n->exports, sizeof(nfsRangeType) * (n->num+1));
      bzero(&n->exports[n->num], sizeof(nfsRangeType));
      n->exports[n->num].prefix = strdup(prefix);
      n->exports[n->num].start = 99999;
      n->exports[n->num].minSizeGB = 999999;
      found = n->num;
      n->num++;
    }
    
    if (n->exports[found].start > suffix) n->exports[found].start = suffix;
    if (n->exports[found].end < suffix) n->exports[found].end = suffix;

  }
}


void nfsExportsFree(nfsRangeExports **nin) {
  nfsRangeExports *n = *nin;
  if (n) {
    for (int i = 0; i < n->num; i++) {
      free(n->exports[i].prefix);
    }
    free(n->exports);
    free(n);
    *nin=NULL;
  }
}


void nfsCheckMissing(nfsRangeExports *n) {

  for (int i = 0; i < n->num; i++) {
    char tryopen[PATH_MAX];
    for (int j = 1; j <= n->exports[i].end; j++) {
      sprintf(tryopen, "%s-%d", n->exports[i].prefix, j);
      FILE *fp = fopen(tryopen, "r");
      if (fp == NULL) {
	n->exports[i].missing++;
      } else {
	fclose(fp);
      }
      
      struct statvfs stat;
      int ret = statvfs(tryopen, &stat);
      if (ret == 0) {
	long sz =   ((long)stat.f_bsize * stat.f_blocks) / 1000000;
	long fre = ((long)stat.f_bsize * stat.f_bfree) / 1000000;
	if (sz < n->exports[i].minSizeGB) n->exports[i].minSizeGB = sz;
	if (sz > n->exports[i].maxSizeGB) n->exports[i].maxSizeGB = sz;
	n->exports[i].sumSizeGB += sz;
	n->exports[i].sumFreeGB += fre;
      }
    }
    


    
    if (n->exports[i].start > 1) {
      fprintf(stderr,"*warning* missing low drives. setting start to 1\n");
      n->exports[i].start = 1;
    }

    // check /proc/mounts
    // if there, get type from underlying block device
    // otherwise get the device under / and return that.
  }
}



char *nfsExportsKV(nfsRangeExports *n) {
  nfsCheckMissing(n);
  fprintf(stderr,"nfs-size;%d ", n->num);
  for (int i = 0; i < n->num; i++) {
    fprintf(stderr,"nfs:%s|start:%d|end:%d|missing%d ", n->exports[i].prefix, n->exports[i].start, n->exports[i].end, n->exports[i].missing);
  }
  fprintf(stderr,"\n");
  return NULL;
}

char *nfsExportsJSON(nfsRangeExports *n) {
  nfsCheckMissing(n);
  printf("{ \"nfsexports\": [ ");
  for (int i = 0;i < n->num;i++) {
    if (i > 0) printf(",");
    printf("{ \"prefix\":\"%s\",", n->exports[i].prefix);
    printf("  \"start\":%d,", n->exports[i].start);
    printf("  \"end\":%d,", n->exports[i].end);
    printf("  \"missing\":%d,", n->exports[i].missing);
    printf("  \"type\":\"hdd\",");
    printf("  \"minSizeGB\":%ld,", n->exports[i].minSizeGB);
    printf("  \"maxSizeGB\":%ld,", n->exports[i].maxSizeGB);
    printf("  \"sumSizeGB\":%ld,", n->exports[i].sumSizeGB);
    printf("  \"sumFreeGB\":%ld", n->exports[i].sumFreeGB);
    printf("}\n");
  }
  printf(" ]\n");
  printf("}\n");

  return NULL;
}


