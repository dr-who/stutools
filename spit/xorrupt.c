#define _DEFAULT_SOURCE

#include <string.h>
#include <signal.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <malloc.h>
#include <assert.h>

/**
 * xorrupt.c
 *
 * a byte at a time device XOR corruptor
 *
 */

int verbose = 0;
int keepRunning = 1;

void intHandler(int d)
{
  if (d) {}
  keepRunning = 0;
}

unsigned char *oldBytes = NULL;
int fd;

size_t maxPositions = 0;
size_t *positions = NULL;
int needtorestore = 0;

void addToPositions(size_t pos) {
  fprintf(stderr,"*info* device position[#%zd]: %zu\n", maxPositions+1, pos);
  positions = realloc(positions, (maxPositions+1) *sizeof(size_t));
  positions[maxPositions] = pos;

  oldBytes = realloc(oldBytes, (maxPositions+1) * sizeof(unsigned char));
  oldBytes[maxPositions] = 0;
  maxPositions++;
}

void freePositions() {
  if (positions) free(positions);
  positions = NULL;
  if (oldBytes) free(oldBytes);
  oldBytes = NULL;
}





void storeBytes() {
  for (size_t i = 0; i < maxPositions; i++) {
    size_t pos = positions[i];
    int r = pread(fd, oldBytes + i, 1, pos);
    if (r <= 0) {
      fprintf(stderr,"*error* offset[%zd] did not return a value\n", pos);
      exit(1);
    }
    assert(r == 1);
    fprintf(stderr,"storing %zd: ASCII %d ('%c')\n", pos, oldBytes[i], oldBytes[i]);
  }
}






void perturbBytes(double prob) {
  for (size_t i = 0; i <maxPositions; i++) {
    if (drand48() < prob) {
      size_t pos = positions[i];
      unsigned char newv = 255 ^ oldBytes[i];
      int r = pwrite(fd, (void*)&newv, 1, pos);
      needtorestore = 1;
      assert(r == 1);
      fprintf(stderr,"overwriting %zd: ASCII %d ('%c')\n", pos, newv, newv);
    }
  }
}


void restoreBytes() {
  for (size_t i = 0; i < maxPositions; i++) {
    size_t pos = positions[i];
    
    fprintf(stderr,"restoring %zd: ASCII %d ('%c')... ", pos, oldBytes[i], oldBytes[i]);
    int r = pwrite(fd, oldBytes + i, 1, pos);
    assert(1==r);
    
    unsigned char check;
    r = pread(fd, (void*)&check, 1, pos);
    assert(check == oldBytes[i]);
    if (r==1) {
      fprintf(stderr,"verified the restoration\n");
    }
  }
  needtorestore = 0;
}



int main(int argc, char *argv[])
{
  int opt;

  char *device = NULL;
  size_t pos = 0;
  size_t repeat = 0;
  size_t runtime = 60;
  double probability = 1;
  size_t seed = 42;

  optind = 0;
  while ((opt = getopt(argc, argv, "f:p:t:rP:R:")) != -1) {
    switch (opt) {
    case 'p':
      if (strchr(optarg,'G') || strchr(optarg,'g')) {
	pos = (size_t)(1024.0 * 1024.0 * 1024.0 * atof(optarg));
      } else if (strchr(optarg,'M') || strchr(optarg,'m')) {
	pos = (size_t)(1024.0 * 1024.0 * atof(optarg));
      } else if (strchr(optarg,'K') || strchr(optarg,'k')) {
	pos = (size_t)(1024 * atof(optarg));
      } else {
	pos = (size_t)atol(optarg);
      }
      addToPositions(pos);
      break;
    case 'R':
      seed = atoi(optarg);
      fprintf(stderr,"*info* seed: %zd\n", seed);
      break;
    case 'P':
      probability = atof(optarg);
      if (probability < 0) probability = 0;
      else if (probability > 1) probability = 1;
      fprintf(stderr,"*info* probability of XOR is %.1lf [0, 1)\n", probability);
      break;
    case 'r':
      repeat = 1;
      break;
    case 'f':
      device = strdup(optarg);
      break;
    case 't':
      runtime = atoi(optarg);
      break;
    default:
      exit(1);
      break;
    }
  }

  if (!device) {
    fprintf(stderr,"*info* xorrupt -f device [-p position ... -p position] [-t time]\n");
    fprintf(stderr,"\nTemporarily XOR a byte at an offset position, sleep then restore\n");
    fprintf(stderr,"\nOptions:\n");
    fprintf(stderr,"   -f device  specifies a block device\n");
    fprintf(stderr,"   -p offset  the block device offset [defaults to byte 0]\n"); 
    fprintf(stderr,"   -p 2k      can use k,M,G units\n");
    fprintf(stderr,"   -p 4M      can use k,M,G units\n");
    fprintf(stderr,"   -p 10G     can use k,M,G units\n");
    fprintf(stderr,"   -P prob    the probability [0, 1) of applying the XOR [defaults to %.1lf)\n", probability);
    fprintf(stderr,"   -R seed    sets the seed [defaults to %zd\n", seed);
    fprintf(stderr,"   -r         repeat forever (usually with -t 2 say)\n");
    fprintf(stderr,"   -t time    the time until restore [defaults to %zd]\n", runtime);
    fprintf(stderr,"\nExamples:\n");
    fprintf(stderr,"  ./xorrupt -f /dev/sdc -p 0 -p 4k -p 80\n");
    fprintf(stderr,"  ./xorrupt -f /dev/sdc -p 0 -p 4k -p 80 -r -t 2\n");
    fprintf(stderr,"  ./xorrupt -f /dev/sdc -p 0 -p 4k -p 80 -r -t 2 -P 0.5\n");
    fprintf(stderr,"\nNote:\n");
    fprintf(stderr,"  Control-C can be used to cancel the timer. The byte is still restored\n");
    exit(1);
  }

  signal(SIGTERM, intHandler);
  signal(SIGINT, intHandler);
  srand48(seed);

  fd = open(device, O_RDWR| O_SYNC, S_IRUSR | S_IWUSR);
  if (fd <= 0) {
    perror(device);
  } else {

    storeBytes();
    
    fprintf(stderr,"*info* pausing for %zd seconds (or control-c), then restore\n", runtime);
    while (keepRunning) {
      perturbBytes(probability);
      
      sleep(runtime);
      if (repeat == 0) {
	break;
      } else {
	restoreBytes();
	sleep(runtime);
	fprintf(stderr,"\n");
      }
    }
  }

  if (needtorestore) restoreBytes();

  if (device) free(device);

  freePositions();
  
  fflush(stderr);

  exit(0);
}
