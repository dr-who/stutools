#define _GNU_SOURCE

#include <stdlib.h>
#include <stdio.h>
#include <assert.h>
#include <ctype.h>

#include "cigar.h"

void cigar_init(cigartype *c) {
  assert(c);
  c->len = 0;
  c->types = NULL;
  c->rand = 0.5;
}

void cigar_setrwrand(cigartype *c, double d) {
  c->rand = d;
}

void cigar_free(cigartype *c) {
  assert(c);
  c->len = 0;
  if (c->types) free(c->types);
  c->types = NULL;
}

size_t cigar_len(const cigartype *c) {
  return c->len;
}

void cigar_dump(const cigartype *c, FILE *fp) {
  for (size_t i = 0; i < cigar_len(c); i++) {
    fputc(c->types[i], fp);
  }
}


void cigar_add(cigartype *c, const size_t len, const char ty) {
  c->types = realloc(c->types, (c->len + len) * sizeof(char));
  for (size_t i = 0 ; i < len; i++) {
    /*    if (ty == 'X') { // X is random
      c->types[c->len + i] 
      if (drand48() <= c->rand) {
	c->types[c->len + i]= 'R';
      } else {
	c->types[c->len + i]= 'W';
      }
      } else {*/
    c->types[c->len + i]= ty;
    // }
  }
  c->len += len;
}


char cigar_at(const cigartype *c, const size_t pos) {
  assert(c);
  assert(pos < c->len);
  return c->types[pos];
}
  


int cigar_parse(cigartype *c, char *string) {
  char *start = string;
  char *end = start;

  while (end && *end) {
    long l = 0;
    if (start && ((*start == '~') || (*start == '@') || (*start == ':') || (*start == '%'))) {
      if (*start == '~') {
	l = (lrand48() % 10) + 1;
      } else if (*start == '@') {
	l = (lrand48() % 90) + 11;
      } else if (*start == ':') {
	l = (lrand48() % 900) + 101;
      } else if (*start == '%') {
	l = (lrand48() % 9000) + 1001;
      } else {
	assert(0);
	//	l=1;
      }
      
      //            fprintf(stderr,"*info* found a %c mapping to %ld\n", *start, l);
      end = start+1;
    } else {
      l = strtol(start, &end, 10);
    }
    if (l < 0) {
      fprintf(stderr,"*warning* only positive values allowed (%ld). Converting to positive.\n", l);
      if (l < 0) l = -l;
    } else if (l == 0) {
      l = 1;
    }
    if (!end || !(*end)) {
      fprintf(stderr,"*error* missing type (%ld?), truncating.\n", l);
      return -2; // problem
    }
    char e = toupper(*end);
    end++;
    //    fprintf(stderr,"'%ld' '%c'\n", l, e);
    cigar_add(c, l, e);
    //    if (*end == 0) break;
    start = end;
  }
  if (cigar_len(c) > 0) {
    return 0; // all good
  } else {
    return -1; // empty
  }
}

