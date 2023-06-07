
#include "dns.h"
#include "utils.h"

int keepRunning = 1;

int main() {
  dnsServersType d;
  dnsServersInit(&d);

  dnsServersAdd(&d, "1.1.1.1");
  dnsServersAdd(&d, "1.1.1.1");
  dnsServersAdd(&d, "8.8.8.8");
  dnsServersAdd(&d, "8.8.4.4");
  dnsServersAddFile(&d, "/etc/resolv.conf", "nameserver");

  dnsServersDump(&d);

  numListType *lookup = calloc(dnsServersN(&d), sizeof(numListType)); assert(lookup);

  const size_t N = dnsServersN(&d);

  for (size_t i =0; i < N; i++) {
    nlInit(&lookup[i], 10000);
  }

  for (size_t i =0; i < N; i++) {
    //    printf("dns server: %s\n", d.dnsServer[i]);
    
    double start = timeAsDouble();
    dnsLookupAServer("example.com", d.dnsServer[i]);
    double end = timeAsDouble();
    nlAdd(&lookup[i], 1000.0 * (end - start));
  }

  for (size_t i =0 ;i < N; i++) {
    printf("dns server: %-12s\tmean %6.2lf ms\n", d.dnsServer[i], nlMean(&lookup[i]));
  }

  for (size_t i =0 ;i < N; i++) {
    nlFree(&lookup[i]);
  }
  
  
  dnsServersFree(&d);

  return 0;
}
