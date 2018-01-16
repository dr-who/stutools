#include <stdio.h>
#include "cigar.h"

int main(int argc, char *argv[]) {
  if (argc < 2) {
    fprintf(stderr,"./cigarTest 10.20-30X\n");
    fprintf(stderr,"./cigarTest 100R\n");
    fprintf(stderr,"./cigarTest 100R_100W_\n");
  } else {
    cigartype c;
    cigar_init(&c);
    
    cigar_setrwrand(&c, 0.5);
    if (cigar_parse(&c, argv[1]) == 0) {
      cigar_dump(&c, stdout);
      fputc('\n', stdout);
    }
    
    cigar_free(&c);
  }
  
  return 0;
}
