
#include "dns.h"
#include "utils.h"

int keepRunning = 1;

int main() {
  char *servers[]={"1.1.1.1", "8.8.8.8", "9.9.9.9", "8.8.4.4", "202.37.129.2", "202.37.129.3"};
  double lookup[6];

  for (size_t i =0 ;i < sizeof(lookup)/sizeof(lookup[0]); i++) {
    printf("dns server: %s\n", servers[i]);
    
    double start = timeAsDouble();
    dnsLookupAServer("example.com", servers[i]);
    double end = timeAsDouble();
    lookup[i] = 1000.0 * (end - start);
  }

  for (size_t i =0 ;i < 6; i++) {
    printf("dns server: %-12s\t%6.2lf ms\n", servers[i], lookup[i]);
  }




  return 0;
}
