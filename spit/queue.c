#include <malloc.h>
#include <stddef.h>
#include <unistd.h>
#include <string.h>

#include "queue.h"

queueType * queueInit() {
  queueType *p = calloc(1, sizeof(queueType));
  return p;
}

void queueAdd(volatile queueType *q, char *s) {
			   
  q->elements[q->head] = strdup(s);
  q->head++;
  if (q->head > 1024) q->head =0;
  while (q->head == q->tail) { //collision
    printf("tail collision\n");
    sleep(1);
  }
}

// free result
char * queuePop(volatile queueType *q) {
  char *r = q->elements[q->tail];
  q->elements[q->tail] = NULL;
  q->tail++;
  if (q->tail > 1024) q->tail = 0;
  return r;
}

int queueCanPop(volatile queueType *q) {
  if (q->head != q->tail) {
    return 1;
  } else {
    return 0;
  }
}



