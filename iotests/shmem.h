#ifndef _SHMEM_H
#define _SHMEM_H

#define SHMEMNAME "stutools"

typedef struct {
  int created;
} shmemType;
  

void shmemInit(shmemType *s);
void shmemFree(shmemType *s);


#endif


