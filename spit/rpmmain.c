

#include <stdio.h>

#include "rpn.h"


int main() {
  rpmItem queue[4];
  queue[0].num = 1; queue[0].op = '#';
  queue[1].num = 5; queue[1].op = '#';
  queue[2].num = 6; queue[2].op = '#';
  queue[3].num = 0; queue[3].op = '+';
  queue[4].num = 0; queue[4].op = '*';



  rpmStack s;

  rpmInit(&s, 10);

  for (size_t i = 0; i <5; i++) {
    if (queue[i].op == '#') {
      rpmAdd(&s, queue[i].num);
    } else {
      rpmOp(&s, queue[i].op);
    }
  }
  
  rpmFree(&s);

  return 0;

}

    
