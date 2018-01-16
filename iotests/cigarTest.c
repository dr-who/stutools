#include <stdio.h>
#include "cigar.h"

int main(int argc, char *argv[]) {
  cigartype c;
  cigar_init(&c);

  cigar_setrwrand(&c, 0.5);
  if (cigar_parse(&c, argv[1]) == 0) {
    cigar_dump(&c, stdout);
  }
  
  cigar_free(&c);
  
  return 0;
}
