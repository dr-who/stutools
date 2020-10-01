#ifndef _LIST_H
#define _LIST_H

typedef struct {
  int len;
  long *values;
  int iterate;
} listtype;

void listConstruct(listtype *l);

void listAdd(listtype *l, long value);
void listAddString(listtype *l, char *string);

void listDump(listtype *l);

void listDestroy(listtype *l);


void listIterateStart(listtype *l);
int listNext(listtype *l, long *value);

#endif

