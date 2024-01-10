
#include "blockdevices.h"

int keepRunning = 1;

int main() {
  blockDevicesType *bd = blockDevicesInit();

  blockDevicesScan(bd);
  blockDevicesScan(bd);
  blockDevicesScan(bd);

  for (size_t i = 0; i < bd->num; i++) {
    char *s = keyvalueDumpAsString(bd->devices[i].kv);
    printf("%s\n", s);
    free(s);
  }

  size_t hddsize = 0;
  size_t numHDD = blockDevicesCount(bd, "HDD", &hddsize);
  printf("numHDD: %zd, size = %.1lf GB\n", numHDD, hddsize / (double)1000.0 / 1000.0 / 1000.0);
  
  blockDevicesFree(bd);
  return 0;
}
