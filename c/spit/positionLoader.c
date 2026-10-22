#define _POSIX_C_SOURCE 200809L
#define _DEFAULT_SOURCE
#define _GNU_SOURCE

#include <stdlib.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>


#include "positions.h"
#include "jobType.h"
#include "utils.h"
#include "aioRequests.h"

int verbose = 0;
int keepRunning = 1;

int main(int argc, char *argv[]) {

    jobType j;
    jobInit(&j);

    positionContainer pc;
    positionContainerInit(&pc, (size_t) (timeAsDouble() * 1000));

    size_t numberoflineerrors = 0;
    for (int i = 2; i < argc; i++) {
        size_t added = positionContainerAddLinesFilename(&pc, &j, argv[i], &numberoflineerrors);
        fprintf(stderr, "loaded %zd lines from %s (line errors %zd)\n", added, argv[i], numberoflineerrors);
    }
    positionContainerCollapse(&pc);

    for (size_t i = 0; i < pc.sz; i++) {
        if (pc.positions[i].action == 'W') {
            pc.positions[i].action = 'R';
            pc.positions[i].verify = 1;
        } else {
            pc.positions[i].action = 'S'; // skip
            pc.positions[i].verify = 0;
        }
    }

    positionContainerInfo(&pc);
    positionContainerDump(&pc, 10);

    size_t ios, trb, twb, ior;
    char *fn = argv[1];
    fprintf(stderr, "*info* validating positions against '%s'\n", fn);
    int fd = open(fn, O_RDONLY | O_DIRECT);
    if (fd < 0) {
        perror(fn);
        exit(-1);
    }
    aioMultiplePositions(&pc, pc.sz, timeAsDouble() + 9e9, 0, 32, 4096, 0, 0, 4096, &ios, &trb, &twb, 1, 0, pc.sz, NULL,
                         1, fd, 0, &ior, 0, NULL, NULL, 1, 0, 0, 1, 0);


    close(fd);

    if (pc.sz > 0) {
        positionContainerFree(&pc);
        jobFree(&j);
    }

    return 0;
}
