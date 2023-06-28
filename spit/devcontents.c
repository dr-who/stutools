#define _POSIX_C_SOURCE 200809L


#include "jobType.h"
#include "sha-256.h"
#include <fcntl.h>
#include <stdlib.h>
#include <stdio.h>
#include <assert.h>

#include "linux-headers.h"

/**
 * devcontents
 *
 * iterate through a device, displaying the contents
 *
 */
#include <stdlib.h>
#include <string.h>

#include "positions.h"
#include "utils.h"
#include "diskStats.h"

#define DEFAULTTIME 10

int verbose = 0;
int keepRunning = 1;


void analyse(unsigned char *buf, int low, int high, int *m, int *x, int *range) {
    int min = 256;
    int max = 0;

    for (int i = low; i < high; i++) {
        if (buf[i] > max) max = buf[i];
        if (buf[i] < min) min = buf[i];
    }
    *m = min;
    *x = max;
    *range = max - min + 1;
}


int main(int argc, char *argv[]) {
    int opt;

    char *device = NULL;
    size_t blocksize = 4 * 1024, width = 70;
    size_t startAt = 0 * 1024 * 1024, finishAt = 1024L * 1024L * 1024L * 1;
    int showsha256 = 0;
    float showentropy = 9e9;
    size_t oneposition = 0;

    optind = 0;
    while ((opt = getopt(argc, argv, "hG:g:w:b:f:se:p:")) != -1) {
        switch (opt) {
	    case 'h':
	        device = NULL;
	        break;
            case 'b':
                blocksize = alignedNumber(atoi(optarg), 512);
                break;
	    case 'p':
	        oneposition = atol(optarg);
		startAt = alignedNumber(oneposition, blocksize);
		finishAt = alignedNumber(startAt + blocksize, blocksize);
		fprintf(stderr,"*info* position %zd\n", oneposition);
	        break;
            case 'e':
                showentropy = atof(optarg);
                break;
            case 'f':
                device = optarg;
                break;
            case 'G':
                if (strchr(optarg, 'K') || strchr(optarg, 'k')) {
                    finishAt = (size_t) (1024.0 * atof(optarg));
                } else if (strchr(optarg, 'M') || strchr(optarg, 'm')) {
                    finishAt = (size_t) (1024.0 * 1024.0 * atof(optarg));
                } else {
                    finishAt = (size_t) (1024.0 * 1024.0 * 1024.0 * atof(optarg));
                }
                //      fprintf(stderr,"*info* finish at %zd (%.4lf GiB, %.3lf MiB)\n", finishAt, TOGiB(finishAt), TOMiB(finishAt));
                break;
            case 'g':
                if (strchr(optarg, 'M') || strchr(optarg, 'm')) {
                    startAt = (size_t) (1024.0 * 1024.0 * atof(optarg));
                } else {
                    startAt = (size_t) (1024.0 * 1024.0 * 1024.0 * atof(optarg));
                }
                //      fprintf(stderr,"*info* start at %zd (%.4lf GiB, %.3lf MiB)\n", startAt, TOGiB(startAt), TOMiB(startAt));
                break;
            case 's':
                showsha256 = 1;
                break;
            case 'w':
                width = atoi(optarg);
                fprintf(stderr, "*info* display contents width = %zd\n", width);
                break;
            default:
                exit(1);
                break;
        }
    }

    // align the numbers to the blocksize
    startAt = alignedNumber(startAt, blocksize);
    finishAt = alignedNumber(finishAt, blocksize);
    if (finishAt < startAt) {
        fprintf(stderr, "*error* finish is less than the start position\n");
    }

    if (!device) {
        fprintf(stderr, "Usage:\n  devcontents -f /dev/device [options...)\n");
	//        fprintf(stderr, "\nDisplay the contents of a block device\n");
        fprintf(stderr, "\nOptions:\n");
        fprintf(stderr, "   -f dev    specify the device\n");
        fprintf(stderr, "   -b n      the block size step (defaults to %zd bytes)\n", blocksize);
        fprintf(stderr, "   -p pos    dump a single position\n");
        fprintf(stderr, "   -g n      starting at n GiB (defaults byte 0)\n");
        fprintf(stderr, "   -g 16M    starting at 16 MiB\n");
        fprintf(stderr, "   -G n      finishing at n GiB (defaults to 1 GiB)\n");
        fprintf(stderr, "   -G 32M    finishing at 32 MiB\n");
        fprintf(stderr, "   -s        show SHA-256 of each block\n");
        fprintf(stderr, "   -w n      first n bytes per block to display (defaults to %zd)\n", width);
        fprintf(stderr, "   -e val    only print lines when entropy < val\n");
        exit(1);
    }



    size_t ioErrors = 0;

    int fd = open(device, O_RDONLY, S_IRUSR | S_IWUSR);
    if (fd < 0) {
        perror(device);
    } else {
        const size_t blockDevSize = blockDeviceSizeFromFD(fd);
	if (blockDevSize < finishAt) finishAt = blockDevSize; // don't finish past the end of the device

	fprintf(stderr, "*info* aligned start:  0x%lx (%zd, %.3lf MiB, %.4lf GiB)\n", startAt, startAt, TOMiB(startAt),
		TOGiB(startAt));
	fprintf(stderr, "*info* aligned finish: 0x%lx (%zd, %.3lf MiB, %.4lf GiB)\n", finishAt, finishAt, TOMiB(finishAt),
		TOGiB(finishAt));


      
        unsigned char *buf = NULL;
        buf = malloc(blocksize);
        if (!buf) {
            fprintf(stderr, "*warning* OOM\n");
            exit(1);
        }

        int firstgap = 0;

        fprintf(stdout, "%9s\t%6s\t%8s\t%6s\t%6s\t%6s\t%6s\t%s\t%s\n", "pos", "p(MiB)", "pos", "p%256K", "min", "max",
                "range", "bps", "contents...");

        unsigned char *pbuf = NULL;
        pbuf = malloc(width + 1);
        if (!pbuf) {
            fprintf(stderr, "*error* OOM\n");
            exit(1);
        }
        char timestring[80];
        const double statictime = timeAsDouble();
        uint8_t hashresult[SIZE_OF_SHA_256_HASH];
	
        for (size_t pos = startAt; pos < finishAt; pos += blocksize) {
            int min = 0, max = 0, range = 0;
            int r = pread(fd, buf, blocksize, pos);
            if (r <= 0) {
	        fprintTimePrefix(stderr);
	        fprintf(stderr,"*error* I/O error at position %zd\n", pos);
		ioErrors++;
                perror(device);
		continue;
            }
            analyse(buf, 0, blocksize, &min, &max, &range);

            if (showsha256) {
                calc_sha_256(hashresult, (void *) buf, blocksize);
            }

            if (showsha256 || (max > 1)) {
                size_t *codedpos = (size_t *) buf;
                size_t *codedtime = (size_t *) (buf + sizeof(size_t));
                size_t spit = 0;
                timestring[0] = 0;
                if ((pos == *codedpos) && (pos != 0)) {
                    spit = 1;
                    sprintf(timestring, "%.1lf secs ago", statictime - (*codedtime) / 10.0);
                }
                memcpy(pbuf, buf, width);
                pbuf[width] = 0;
                for (size_t j = 0; j < width; j++) {
                    if (pbuf[j] < 32) pbuf[j] = '_';
                    if (pbuf[j] >= 127) pbuf[j] = ' ';
                }
                if (showsha256) {
                    for (size_t j = 0; j < SIZE_OF_SHA_256_HASH; j++) {
                        fprintf(stdout, "%02x", hashresult[j]);
                    }
                    fprintf(stdout, "\t");
                }
                double bps = 0;
                if (1) {
                    bps = entropyTotalBits(buf, blocksize, 1) / blocksize;
                }
                if (bps < showentropy)
                    fprintf(stdout, "%09zx\t%6.1lf\t%8zd\t%6zd\t%6d\t%6d\t%6d\t%.4lf\t%s %s %s\n", pos, TOMiB(pos), pos,
                            pos % (256 * 1024), min, max, range, bps, pbuf, (spit == 1) ? "*spit*" : "", timestring);
                firstgap = 1;
            } else {
                if (firstgap) {
                    fprintf(stdout, "...\n");
                    firstgap = 0;
                }
            }
        }
        if (pbuf) {
            free(pbuf);
        }

        close(fd);
        if (buf) {
            free(buf);
        }
    }

    if (ioErrors) {
      fprintTimePrefix(stderr);
      fprintf(stderr,"*error* there were %zd I/O errors\n", ioErrors);
    }

    exit(0);
}


