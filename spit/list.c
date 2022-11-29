#include "list.h"

#include "linux-headers.h"

#include <string.h>
#include <stdlib.h>

void listConstruct(listtype *l)
{
  l->len = 0;
  l->values = NULL;
  l->iterate = -1;
}

void listDestroy(listtype *l)
{
  if (l->values)
    free(l->values);
  listConstruct(l);
}

void listDump(listtype *l)
{
  for (int i = 0; i < l ->len ; i++) {
    fprintf(stderr,"[%d] %ld\n", i, l->values[i]);
  }
}

void listAdd(listtype *l, long value)
{
  l->values = realloc(l->values, sizeof(long) * (l->len+1));
  l->values[l->len] = value;
  l->len++;
}

void listAddString(listtype *l, char *string)
{
  char *pos = string;
  char *endp = pos;
  long startrange = -1;
  while (pos && (*pos) && (pos - string < (int)strlen(string))) {
    long v = strtol(pos, &endp, 10);
    if (endp && (*endp) && (*endp == '-')) {
      startrange = l->len;
      listAdd(l, v);
      //            fprintf(stderr,"range %ld\n", startrange);
    } else {
      if ((startrange >= 0) && (v >= l->values[startrange])) {
        //	fprintf(stderr,"range [%zd to %ld]\n", l->values[startrange], v);
        for (long i2 = l->values[startrange]+1; i2 <= v; i2++) {
          listAdd(l, i2);
        }
        startrange = -1;
      } else {
        // just a single value
        listAdd(l, v);
      }
    }
    pos = endp + 1;
  }
}


void listIterateStart(listtype *l)
{
  l->iterate = 0;
}

// returns 1 if valid value
int listNext(listtype *l, long *value, int cyclic)
{
  if (l->iterate == -1) {
    fprintf(stderr,"*error* listNext called without listIterateStart\n");
    return 0;
  }
  if (l->iterate < l->len) {
    *value = l->values[l->iterate];
  } else {
    return 0;
  }
  l->iterate++;

  if (cyclic) {
    if (l->iterate >= l->len)
      l->iterate = 0;
  }
  return 1;
}


/*int main () {

  listtype l;
  listConstruct(&l);
  listAddString(&l, "3-8,10,-11,-100--98,5-4,4-5,1,2");

  listIterateStart(&l);
  long v;
  while (listNext(&l, &v, 0)) {
    fprintf(stderr,"value %ld\n", v);
  }

  listDestroy(&l);

  return 0;
}
*/
