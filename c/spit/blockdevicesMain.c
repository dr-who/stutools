
#include "blockdevices.h"

int keepRunning = 1;

int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;
  blockDevicesType *bd = blockDevicesInit();
  blockDevicesScan(bd);

  for (size_t i = 0; i < bd->num; i++) {
    char *s = keyvalueDumpAsString(bd->devices[i].kv);
    fprintf(stderr, "%s\n", s);
    free(s);
  }

  size_t size = 0;
  size_t num = blockDevicesCount(bd, "SSD", &size);
  fprintf(stderr,"SSD: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "HDD", &size);
  fprintf(stderr,"HDD: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "HDD,SSD", &size);
  fprintf(stderr,"HDD,SSD: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "Volatile-RAM", &size);
  fprintf(stderr,"Volatile-RAM: num: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "Virtual", &size);
  fprintf(stderr,"Virtual: num: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "Removable", &size);
  fprintf(stderr,"Removable: num: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);
  num = blockDevicesCount(bd, "NVMe", &size);
  fprintf(stderr,"NVMe: num: %zd, size = %.1lf GB\n", num, size / (double)1000.0 / 1000.0 / 1000.0);

  fprintf(stdout, "%s\n", blockDevicesAllJSON(bd));
  blockDevicesFree(bd);
  return 0;
}
