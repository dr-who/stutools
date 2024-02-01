#include <stdio.h>
#include <stdlib.h>


#include "keyvalue.h"
#include "string.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;

  char *line = NULL;
  size_t len = 1000;
  
  getline(&line, &len, stdin);
  
  keyvalueType *kv = keyvalueInitFromString(line);
  //  printf("loaded checksum %zd\n", kv->checksum);

  size_t ch = keyvalueChecksum(kv);

  if (kv->checksum && (kv->checksum != ch)) {
    printf("bad checksum\n");
    exit(1);
  }
  
  //  printf("calc checksum %zd\n", ch);
       
  char *s = keyvalueDumpAsJSON(kv);
  printf("%s\n", s);
  free(s);
  
  keyvalueFree(kv);
  
  free(line);

  return 0;
}
