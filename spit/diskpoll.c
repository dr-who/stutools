#include <stdlib.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>

#include "utils.h"

int keepRunning = 1;

void usage(float timems) {
    fprintf(stderr, "Usage:\n  diskpoll -f device [-s seconds]\n");
    fprintf(stderr, "\nOptions:\n");
    fprintf(stderr, "  -f d  \tthe device\n");
    fprintf(stderr, "  -s n  \tperform IO every n seconds (default %.0lf)\n", timems / 1000.0);
    fprintf(stderr, "  -q    \tbe quiet\n");
    fprintf(stderr, "  -r    \tseek to random position\n");
}

int main(int argc, char *argv[]) {

    int opt, quiet = 0, random = 0;
    const char *getoptstring = "s:f:qrG:";
    char *device = NULL;

    float timems = 300; // poll every 300ms
    float limitgb = 0;
    size_t stopaftern = (size_t) -1;

    while ((opt = getopt(argc, argv, getoptstring)) != -1) {
        switch (opt) {
            case 's':
                timems = atof(optarg) * 1000.0;
                break;
            case 'f':
                device = strdup(optarg);
                break;
            case 'q':
                quiet = 1;
                break;
            case 'G':
                limitgb = atof(optarg);
                break;
            case 'r':
                random = 1;
                break;
            case 'h':
                usage(timems);
                exit(1);
        }
    }

    if (device == NULL) {
        usage(timems);
        exit(1);
    }

    // now run
    double starttime = timeAsDouble();

    size_t displaycount = 0;
    int fd = open(device, O_RDONLY);
    if (fd < 0) {
        perror(device);
        exit(1);
    }

    size_t devicebytes = blockDeviceSizeFromFD(fd);
    if (!quiet) {
        fprintf(stdout, "*info* opened device '%s', size %.0lf TB\n", device, TOTB(devicebytes));
    }

    if (limitgb > 0) {
        if (limitgb * 1000L * 1000 * 1000 < devicebytes) {
            devicebytes = alignedNumber(limitgb * 1000L * 1000 * 1000, 4096);
        }
    }

    if (!quiet) {
        if (limitgb) {
            fprintf(stderr, "*info device limited to size %.0lf GB\n", TOGB(devicebytes));
        }
    }

    if (random) {
        srand48(0);
    }

    unsigned char buf[4097];


    while (++displaycount <= stopaftern) {

        {
            long long pos = 0;
            if (random) pos = (alignedNumber(drand48() * devicebytes - 4096, 4096));
            assert(pos > 0);

            if (!quiet) {
                fprintf(stdout, "[%.1lf / %zd]: seek on '%s' to pos %lld (%.3lf TB)\n", timeAsDouble() - starttime,
                        displaycount, device, pos, TOTB(pos));
            }

            ssize_t lret = pread(fd, buf, 4096, pos);
            if (lret != 4096) {
                perror(device);
            }
        }

        usleep(timems * 1000.0);

    }

    free(device);
    close(fd);


    exit(0);
}

  
