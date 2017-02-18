#define _GNU_SOURCE
#include <stdlib.h>
#include <stdio.h>
#include <fcntl.h> // for O_CREAT 
#include <string.h>
#include <sys/shm.h>
#include <sys/stat.h>
#include <sys/mman.h>

#include "utils.h"
#include "shmem.h"

void shmemInit(shmemType *s) {
  const int SIZE = 4096;
  char *message0= username();

  /* create the shared memory segment */
  s->created = 0;

  int shm_fd = shm_open(SHMEMNAME, O_RDONLY, 0666);
  if (shm_fd >= 0) {
    fprintf(stderr,"already there\n");
    return;
  }
  close(shm_fd);
  
  shm_fd = shm_open(SHMEMNAME, O_CREAT | O_RDWR, 0666);
  if (shm_fd < 0) {
    fprintf(stderr,"problem\n");
    return;
  }

  /* configure the size of the shared memory segment */
  if (ftruncate(shm_fd,SIZE) != 0) {
    fprintf(stderr,"ftruncate problem\n");
  }

  /* now map the shared memory segment in the address space of the process */
  void *ptr = mmap(0,SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, shm_fd, 0);
  if (ptr == MAP_FAILED) {
    fprintf(stderr, "Map failed\n");
    return ;
    //    exit(1);
  }

  sprintf(ptr,"%lf %s\n", timedouble(), message0);
  ptr += strlen(message0);
  free(message0);
  fprintf(stderr,"created shmem\n");
  s->created = 1;
}



void shmemCheck() {
  const int SIZE = 4096;

  /* open the shared memory segment */
  int shm_fd = shm_open(SHMEMNAME, O_RDONLY, 0666);
  if (shm_fd == -1) {
    // not there
    return ;
  } else {
    fprintf(stderr,"warning: POTENTIAL multiple tests running at once\n");
  }

  /* now map the shared memory segment in the address space of the process */
  void * ptr = mmap(0,SIZE, PROT_READ, MAP_SHARED, shm_fd, 0);
  if (ptr == MAP_FAILED) {
    fprintf(stderr,"error: can't allocate shmem\n");
    // it's not there :)
    exit(1);
  }

  double time = 0;
  char s[SIZE];
  sscanf(ptr, "%lf %s", &time, s);

  fprintf(stderr,"warning: user %s, seconds ago: %.1lf %s\n", s, timedouble() - time, (timedouble() - time < 5) ? " *** NOW *** ": "");
  
}


void shmemFree(shmemType *s) {
  if (s->created) {
    if (shm_unlink(SHMEMNAME) == -1) {
      //    printf("Error removing %s\n",name);
      return ;
    }
    fprintf(stderr,"unlinked\n");
  }
}
