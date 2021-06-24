#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <assert.h>
#include <math.h>

/*
 * Software licensed by Stuart Inglis, Ph.D.
 *
 * Commercial use and derivations require a license (or discussion beforehand)
 * 
 */

// if -1 then byte
// if -4 then 4 byte numbers
// if -8 then 8 byte numbers

#define BUFSIZE (1024*1024)

void analyseAsBits(int bytes) {
  unsigned char *buffer = NULL;

  const size_t bits = 8 * bytes;
  size_t counts[bits], counts1[bits], tot[bits];

  buffer = malloc(BUFSIZE);
  assert(buffer);

  size_t size = 0;
  for (size_t i = 0; i < bits; i++) {
    counts[i] = 0;
    counts1[i] = 0;
    tot[i] = 0;
  }

  size_t sz = 0;
  while ((size = read(fileno(stdin), buffer, bytes)) > 0) {
    assert((int)size == bytes);
    unsigned long thev;
    if (bytes == 4) {
      unsigned int *v = (unsigned int*) buffer;
      thev = (unsigned int) (*v);
    } else if (bytes == 8) {
      unsigned long *v = (unsigned long*) buffer;
      thev = (unsigned long) (*v);
    } else if (bytes == 2) {
      unsigned short *v = (unsigned short*) buffer;
      thev = (unsigned short) (*v);
    } else {
      abort();
    }
      
    //    fprintf(stderr,"number %u\n", *v);
    for (size_t j = 0; j < bits; j++) {
      if (thev & (1L<<j)) {
	counts1[j]++;
      } else {
	counts[j]++;
      }
      tot[j]++;
    }
  }

  sz = 0;
  for (size_t i = 0;i < bits; i++) {
    sz += tot[i];
  }


  if (sz) {
    double entropy = 0;
    //                fprintf(stderr,"size: %ld\n", sz);                                                                                                                                                      
    for (size_t i =0; i < bits; i++) {
      if (counts[i]) {
        double e = counts[i] * (log((counts[i]) * 1.0 / tot[i])) / log(2.0);
	//	fprintf(stderr,"[%zd] %zd %zd %.4lf %.4lf    %.4lf\n", i, counts[i], tot[i], e, entropy, -entropy/counts[i]);
	entropy = entropy - e;
      }
      if (counts1[i]) {
        double e = counts1[i] * (log((counts1[i]) * 1.0 / tot[i])) / log(2.0);
	//	fprintf(stderr,"1[%zd] %zd %zd %.4lf %.4lf   %.4lf\n", i, counts1[i], tot[i], e, entropy, -entropy/counts1[i]);
	entropy = entropy - e;
      }
    }
    double bpc = bits * entropy / sz;
    const double threshold = bits * 0.99;
    //    fprintf(stderr,"entropy %.4lf, %zd\n", entropy, bits);
    fprintf(stderr,"%.7lf bpw: %s\n", bpc, bpc >= threshold ? "RANDOM" : "");
  }
  free(buffer);
}


void analyse1B() {
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
      if (counts[i]) {
	double e = counts[i] * (log((counts[i]) * 1.0 / (sz))) / log(2.0);
	//      fprintf(stderr,"%zd %zd %.4lf %.4lf\n", counts[i], sz, e, entropy);
	entropy = entropy - e;
      }
    }
    double bpc = entropy / sz;
    fprintf(stderr,"%.7lf bpc: %s\n", bpc, bpc > 8*0.99 ? "RANDOM" : "");
  }
  free(buffer);
}  



int main(int argc, char *argv[]) {
  int opt;
  int bytes = 1;

  const char *getoptstring = "1248";

  while ((opt = getopt(argc, argv, getoptstring )) != -1) {
    switch (opt) {
    case '1': 
      bytes = 1;
      break;
    case '2':
      bytes = 2;
      break;
    case '4':
      bytes = 4;
      break;
    case '8':
      bytes = 8;
      break;
    default:
      exit(1);
    }
  }

  if (bytes == 1) {
    analyse1B();
  } else {
    analyseAsBits(bytes);
  }
  
  return 0;
}
