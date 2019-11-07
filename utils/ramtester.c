#include <stdlib.h>

#include <malloc.h>
#include <sys/sysinfo.h>
#include <sys/time.h>
#include <string.h>

size_t freeRAM() {
  struct sysinfo info;
  sysinfo(&info);
  return info.freeram + info.bufferram + info.sharedram;
}

size_t totalswap() {
  struct sysinfo info;
  sysinfo(&info);
  return info.totalswap;
}

inline double timedouble() {
  struct timeval now;
  gettimeofday(&now, NULL);
  double tm = ((double)now.tv_sec * 1000000.0) + now.tv_usec;
  return tm/1000000.0;
}


int main(int argc, char *argv[]) {
  size_t ram = freeRAM(), swap = totalswap();

  if (swap != 0) {
    fprintf(stderr,"*warning* turn off swap with swapoff -a\n");
    exit(1);
  }
  
  fprintf(stderr,"*info* ram to test %zd MiB\n", ram/1024/1024);

  unsigned char *c = NULL;
  do {
    c = malloc(ram);
    if (!c) {
      fprintf(stderr,"No ram in %zd chunk\n", ram);
      ram = ram * 0.99;
      //      exit(1);
    }
  } while (c == NULL);
  fprintf(stderr,"*info* allocated %zd chunk\n", ram);

  size_t errors = 0;
  unsigned char pattern[3] = {0xff, 0x00, 0x01 + 0x04 + 0x10 + 0x40};
  for (size_t pat = 0; pat < 3; pat++ ){
    fprintf(stderr,"*info* setting pattern to %2x\n", pattern[pat]);
    memset(c, pattern[pat], ram);

    double starttime = timedouble();
    double runtime = 5* 60;
    size_t pass = 1;
    while (timedouble() - starttime < runtime) { // 5 mins
      fprintf(stderr,"*info* pass %zd looking for non %2x (%.0lf secs remain)\n", pass++, pattern[pat], runtime - (timedouble() - starttime));
      if (errors) {
	fprintf(stderr,"**ERROR** memory errors: %zd\n", errors);
      }
      unsigned char *p = c;
      for (size_t i = 0; i < ram; i++) {
	if (*p != pattern[pat]) {
	  fprintf(stderr,"error at [%zd]: was %2x instead of %2x\n", i, *p, pattern[pat]);
	  errors++;
	}
	p++;
      }
    }
  }
  free(c);
  
  if (errors)
    return 1;
  else
    return 0;
}
