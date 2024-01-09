
#include "blockdevices.h"

int keepRunning = 1;

int main() {
  blockDevicesType *bd = blockDevicesInit();

  blockDevicesScan(bd);

  for (size_t i = 0; i < bd->num; i++) {
    char *s = keyvalueDumpAsString(bd->devices[i].kv);
    printf("%s\n", s);
    free(s);
  }

  blockDevicesFree(bd);
  return 0;
}
