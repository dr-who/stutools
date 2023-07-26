#include <stdio.h>

#include "simpmail.h"

int main(int argc, char *argv[]) {
  if (argc < 4) {
    printf("usage: simpmail <from> <to> <subject>\n");
    return 1;
  }

  int fd = simpmailConnect("127.0.0.1");

  if (fd > 0) {
    char *from = argv[1];
    char *to = argv[2];
    char *subject = argv[3];
    char body[] = "test email\n";
    simpmailSend(fd, from, to, subject, body);
    simpmailClose(fd);
  }

  return 0;
}
