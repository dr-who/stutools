#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <arpa/inet.h>
#include <time.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <assert.h>

#include "transfer.h"

#include "utils.h"
#include "numList.h"

#include <glob.h>

int keepRunning = 1;

#include "snack.h"

int main() {


    int threads = 100;
    fprintf(stderr, "*info* starting receiver -- using %d ports/threads\n", threads);
    dumpEthernet();

    if (threads < 1) threads = 1;
    // start servers
    startSnack(threads);

    return 0;
}
