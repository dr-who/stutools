
#include "dns.h"
#include "utils.h"

int keepRunning = 1;

int main() {
  double start = timeAsDouble();
  dnsConnect("192.168.5.254");
  double end = timeAsDouble();
  printf("Time: %.2lf ms\n", 1000.0 * (end - start));

  return 0;
}
