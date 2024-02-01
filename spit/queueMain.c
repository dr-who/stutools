#include <stdlib.h>
#include <stdio.h>

#include "queue.h"

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  queueType *q = queueInit();


  queueAdd(q, "stu");
  queueAdd(q, "list");


  char *s;
  while (queueCanPop(q)) {
    s = queuePop(q);
    printf("%s\n", s);
    free(s);
  }


  return 0;
}
