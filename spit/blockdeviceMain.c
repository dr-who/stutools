#include "blockdevice.h"

int main(int argc, char *argv[]) {

  if (argc > 1) {
    for (int i = 1; i < argc; i++) {
      bdType *bd = bdInit(argv[i]);
      if (bd) {
	bdDumpKV(stderr, bd);
      }
      //      bdFree(bd);
    }
  }

  return 0;
}
