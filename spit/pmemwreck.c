#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <assert.h>
#include <errno.h>
#include <string.h>
#include <stdio.h>
#include <sys/mman.h>
#include "utils.h"

#include <regex.h>

int keepRunning = 1;
int verbose = 0;


void help() {
  fprintf(stdout,"Usage: ./pmemwreck [options] /dev/dax0.0\n");
  fprintf(stdout,"\nDescription:\n   utilities for changed pmem contents\n");
  fprintf(stdout,"\nOptions\n");
  fprintf(stdout,"   -4  iterate every 4KiB block, XOR'ing first byte per block\n");
  fprintf(stdout,"   -a  every 4KiB block, changed first 8-mer ASCII string\n");
}
  

int main(int argc, char *argv[])
{

  char *suffix = NULL, *dev = NULL;
  
  int opt = 0, command = 0;
  const char *getoptstring = "4kaf:v";

  while ((opt = getopt(argc, argv, getoptstring )) != -1) {
    switch (opt) {
    case 'f':
      dev = strdup(optarg);
      suffix = getSuffix(dev);
      fprintf(stdout,"*info* device = '%s'\n", dev);
      break;
    case '4':
      command = 4;
      break;
    case 'a':
      command = 1;
      break;
    case 'v':
      verbose++;
      break;
    default:
      break;
    }
  }
  
  if (suffix == NULL || (command == 0)) {
    help();
    exit(1);
  }

  size_t sz = 0;
  
  char sys[1000];
  char *path = "/sys/class/dax/";
  fprintf(stdout,"*info* opening ... %s%s\n", path, suffix);
  sprintf(sys, "%s%s/device/align", path, suffix);

  
  FILE *fp = fopen(sys, "rt");
  if (!fp) {
    // not a real fax device?
    fprintf(stdout,"*error* not a dax?\n");
    exit(1);
  }
  size_t align = 0;
  int ret = fscanf(fp, "%lu", &align);
  fclose(fp);

  fprintf(stdout,"*info* align %zd\n", align);


  
  // size

  sprintf(sys, "%s%s/size", path, suffix);
  free(suffix);
  fp = fopen(sys, "rt");
  if (!fp) {
    // not a real fax device?
    fprintf(stdout,"*error* not a dax?\n");
    exit(1);
  }
  size_t maxsz = 0;
  ret = fscanf(fp, "%lu", &maxsz);
  fclose(fp);

  fprintf(stdout,"*info* max size %zd\n", maxsz);


  maxsz = alignedNumber(maxsz, align);
  if (ret != 1) maxsz = 0;
  if (sz > maxsz) {
    sz = maxsz;
  }
  if (sz == 0) {
    sz = maxsz;
  }
  
  fprintf(stdout,"*info* logical size of device %zd (%.1lf GiB), aligned to %zd\n", maxsz, TOGiB(maxsz), align);
  fprintf(stdout,"*info* opening '%s', wrecking %zd bytes (%.1lf GiB), in 4096 steps\n", dev, sz, TOGiB(sz));


  int fd = open( dev, O_RDWR );
  if( fd < 0 ) {
    perror(dev);
    return 1;
  }

  void * src =  mmap (0, sz, PROT_WRITE, MAP_SHARED, fd, 0);
  if (!src) {
    fprintf(stdout,"*error* mmap failed\n");
    exit(1);
  }
  unsigned char *s = (unsigned char*)src;

  if (command == 4) {
    size_t changes = 0;
    for (unsigned long i = 0; i < sz; i += 4096) {
      if (verbose) fprintf(stderr,"[0x%lx] %d -> %d\n", i, s[i], 255^s[i]);
      s[i] = 255 ^ s[i];
      changes++;
    }
    fprintf(stdout,"*info* commandType '%d': applied %zd changes\n", command, changes);
  } else if (command == 1) { //ascii

    size_t changes = 0;
    const int len = 8; // max match string
    char *mstring = malloc(len + 1);

    for (unsigned long i = 0; i < sz; i += 4096) {

      regex_t regex;
      regmatch_t match;
      int reti;
      
      /* Compile regular expression */
      reti = regcomp(&regex, "[A-z][A-z][A-z][A-z][A-z][A-z][A-z][A-z]", 0);
      if (reti) {
	fprintf(stderr, "Could not compile regex\n");
	exit(1);
      }
      /* Execute regular expression */
      char buf[4097];
      strncpy(buf, (char*)(s+i), 4096);
      buf[4096] = 0;
      reti = regexec(&regex, buf, 1, &match, 0);
      if (!reti) {
	unsigned long pos = i+match.rm_so;

	strncpy(mstring, (char*)(s+pos), len);
	mstring[len] = 0;
	
	if (verbose) fprintf(stderr,"[0x%lx] changing '%s'\n", i, mstring);
	
	for (int z = 0; z < len ; z++) {
	  char t = s[pos + z] + 1;
	  s[pos + z] = t;
	}
      }

      changes++;
      
      regfree(&regex);
    }
    fprintf(stdout,"*info* commandType '%d': applied %zd changes\n", command, changes);

  }

  // unmap
  msync(src, sz, MS_SYNC);
  munmap(0, sz);

  close( fd );
  return 0;
}
