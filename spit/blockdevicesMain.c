
#include "blockdevices.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  blockDevicesType *bd = blockDevicesInit();
  blockDevicesScan(bd);

  for (size_t i = 0; i < bd->num; i++) {
    char *s = keyvalueDumpAsString(bd->devices[i].kv);
    printf("%s\n", s);
    free(s);
  }

  size_t size = 0;
  size_t num = blockDevicesCount(bd, "SSD,HDD", &size);
  printf("SSD: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "HDD", &size);
  printf("HDD: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "HDD,SDD", &size);
  printf("HDD,SSD: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "Volatile-RAM", &size);
  printf("Volatile-RAM: num: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "Virtual", &size);
  printf("Virtual: num: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  
  blockDevicesFree(bd);
  return 0;
}
