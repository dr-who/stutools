#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <assert.h>
#include <math.h>

int verbose = 0;

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
  size_t counts0[bits], counts1[bits], tot[bits];

  buffer = malloc(BUFSIZE);
  assert(buffer);

  size_t size = 0;
  for (size_t i = 0; i < bits; i++) {
    counts0[i] = 0;
    counts1[i] = 0;
    tot[i] = 0;
  }

  size_t sz = 0;
  while ((size = read(fileno(stdin), buffer, bytes)) > 0) {
    if (size != (size_t) bytes)
      continue;
    //    assert((int)size == bytes);
    unsigned long thev;
    if (bytes == 1) {
      unsigned char *v = (unsigned char*)buffer;
      thev = (int) (*v);
    } else if (bytes == 4) {
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
      
    //    fprintf(stderr, "%lx\n", thev);
    for (size_t j = 0; j < bits; j++) {
      if (thev & (1L<<j)) {
	counts1[j]++;
      } else {
	counts0[j]++;
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
      if (counts0[i]) {
	double e = counts0[i] * (log((counts0[i]) * 1.0 / tot[i])) / log(2.0);
	if (verbose) fprintf(stderr,"[b%02zd=0] %3zd %3zd %.4lf %.4lf\n", i, counts0[i], tot[i], e, -entropy/counts0[i]);
	entropy = entropy - e;
      }
      
      if (counts1[i]) {
	double e = counts1[i] * (log((counts1[i]) * 1.0 / tot[i])) / log(2.0);
	if (verbose) fprintf(stderr,"[b%02zd=1] %3zd %3zd %.4lf %.4lf\n", i, counts1[i], tot[i], e, -entropy/counts1[i]);
	entropy = entropy - e;
      }
    }
    double bpc = bits * entropy / sz;
    const double threshold = bits * 0.99;
    //    fprintf(stderr,"entropy %.4lf, %zd\n", entropy, bits);
    fprintf(stdout, "%.7lf bps (compression %.1lfx) %s\n", bpc, bits/bpc, bpc >= threshold ? "RANDOM" : "");
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
	if (verbose) fprintf(stderr,"[0x%2x] %zd %zd %.4lf\n", (unsigned int)i, counts[i], sz, e);
	entropy = entropy - e;
      }
    }
    double bpc = entropy / sz;
    fprintf(stdout, "%.7lf bps (compression %.1lfx) %s\n", bpc, 8/bpc, bpc > 8*0.99 ? "RANDOM" : "");
  }
  free(buffer);
}  



int main(int argc, char *argv[]) {
  int opt;
  int bytes = 0;

  const char *getoptstring = "1248v";

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
    case 'v':
      verbose++;
      break;
    default:
      exit(1);
    }
  }

  if (bytes == 0) {
    analyse1B();
  } else {
    analyseAsBits(bytes);
  }
  
  return 0;
}
