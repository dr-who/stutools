#ifndef _CIGAR_H
#define _CIGAR_H

#include <stdio.h>

typedef struct {
  size_t len;
  size_t *size;
  char   *types;
  double rand;
} cigartype;

void cigar_init(cigartype *c);
void cigar_free(cigartype *c);

size_t cigar_len(const cigartype *c);
void cigar_dump(const cigartype *c, FILE *fp);

void cigar_add(cigartype *c, const size_t len, const char ty);
void cigar_setrwrand(cigartype *c, double d);
int cigar_parse(cigartype *c, char *string);

char cigar_at(const cigartype *c, const size_t pos);


#endif
