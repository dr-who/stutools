#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <assert.h>
#include <math.h>

#define BUFSIZE (1024*1024)
int main() {
  unsigned char *buffer = NULL;
  size_t counts[256];
  
  buffer = malloc(BUFSIZE);
  assert(buffer);

  size_t size = 0; 
  for (size_t i = 0; i < 256; i++) {
    counts[i] = 0;
  }

  size_t sz = 0;
  while ((size = read(fileno(stdin), buffer, BUFSIZE)) > 0) {
    for (size_t i = 0; i < size; i++) {
      unsigned int v = (unsigned int)buffer[i];
      assert (v<256);
      counts[v]++;
    }
    sz += size;
  }

  if (sz) {
    double entropy = 0;
    //            fprintf(stderr,"size: %ld\n", sz);
    for (size_t i =0; i < 256; i++) {
      double e = counts[i] * (log((counts[i]+1) * 1.0 / (sz+1))) / log(2.0);
      //      fprintf(stderr,"%zd %zd %.4lf %.4lf\n", counts[i], sz, e, entropy);
      entropy = entropy - e;
    }
    double bpc = entropy / sz;
    fprintf(stderr,"%.7lf bpc: %s\n", bpc, bpc > 7.9 ? "RANDOM > 7.9" : "");
  }

  return 0;
}
