#ifndef __QUEUE_H
#define __QUEUE_H



typedef struct {
  int head;
  int tail;
  char *elements[1024];
} queueType;

queueType * queueInit();
void queueAdd(volatile queueType *q, char *s);
char * queuePop(volatile queueType *q);
int queueCanPop(volatile queueType *q);

#endif
