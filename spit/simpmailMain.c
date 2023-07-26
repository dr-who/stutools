
#include "simpmail.h"

int main(void){

  int fd = simpmailConnect("127.0.0.1");
  
  simpmailSend(fd, "stuart@example.com", "stuart+test@example.com", "test sub", "cool test");
  simpmailClose(fd);

  return 0;
}
