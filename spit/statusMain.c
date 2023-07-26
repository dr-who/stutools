#include <stdlib.h>
#include <stdio.h>

#include "status.h"

int main() {
  
  
  int httpcode;
  char *s = genStatus(&httpcode);
  printf("Status: %d\r\n", (httpcode == 0) ? 200 : 410);
  printf("\r\n");
  
  if (s) printf("%s\n", s);
  free(s);

  return 0;
}
