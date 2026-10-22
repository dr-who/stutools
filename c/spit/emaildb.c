#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <malloc.h>

#include "emaildb.h"

emaildbType* emaildbInit(void) {
  emaildbType *d = calloc(sizeof(emaildbType), 1);
  return d;
}


void emaildbAdd(emaildbType *e, char *emailaddress) {
  for (size_t i = 0; i < e->len; i++) {
    if (strcasecmp(emailaddress, e->addr[i])==0) {
      // match
      return;
    }
  }
  e->len++;
  e->addr = realloc(e->addr, (e->len) * sizeof(char*));
  e->addr[e->len-1] = strdup(emailaddress);
}

void emaildbDump(emaildbType *e) {
  for (size_t i = 0; i < e->len; i++) {
    printf("%s\n", e->addr[i]);
  }
  fprintf(stderr, "[%zd]\n", e->len);
}

emaildbType * emaildbLoad(FILE *stream) {
  emaildbType * e = emaildbInit();
  
  if (stream) {
    char *line = NULL;
    size_t linelen = 0;
    ssize_t nread;
    
    while ((nread = getline(&line, &linelen, stream)) != -1) {

      if (nread > 1) {
	if (strchr(line, '@')) {
	  // needs an @ at least!
	  line[nread-1] = 0;
	  emaildbAdd(e, line);
	}
      }
    }
    
    free(line);
  } else {
    perror("problem");
  }
  
  return e;
}
  

void emaildbFree(emaildbType *e) {
  for (size_t i = 0; i < e->len; i++) {
    free(e->addr[i]);
  }
  free(e->addr);
  memset(e, 0, sizeof(emaildbType));
}
