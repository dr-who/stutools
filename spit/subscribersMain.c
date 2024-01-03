
#include "subscribers.h"

int main(int argc, char *argv[]) {

  connType *c = subscribersOpen();

  subscribersClose(c);
  return 0;
}

