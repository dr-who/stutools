#define _DEFAULT_SOURCE
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <errno.h>
#include <assert.h>

#include "devices.h"
#include "utils.h"
#include "diskStats.h"
#include "procDiskStats.h"

extern int keepRunning;

deviceDetails *addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs) {

    unsigned int major, minor;
    majorAndMinorFromFilename(fn, &major, &minor);
    char *fnserial = getFieldFromUdev(major, minor, "E:ID_SERIAL=");

    for (size_t i = 0; i < *numDevs; i++) {
        deviceDetails *dcmp = &(*devs)[i];
        //    fprintf(stderr,"looking at %s %p\n", dcmp->devicename, dcmp);

        unsigned int major2, minor2;
        majorAndMinorFromFilename(dcmp->devicename, &major2, &minor2);
        char *fnserial2 = getFieldFromUdev(major2, minor2, "E:ID_SERIAL=");

        if (strcmp(fn, dcmp->devicename) == 0) {
            fprintf(stderr, "*warning* ignoring '%s', it duplicates a prior device\n", fn);
            return NULL;
            //      return &(*devs)[i];
        }
        if (fnserial && (strlen(fnserial) >= 5) && (strcmp(fnserial, fnserial2) == 0)) {
            fprintf(stderr, "*warning* ignoring '%s', it shares a serial number '%s' with '%s'\n", fn, fnserial2,
                    dcmp->devicename);
            return NULL;
            //      return &(*devs)[i];
        }
        free(fnserial2);
    }
    free(fnserial);

    // Get the realpath
    char *base_path = realpath(fn, NULL);
    if (base_path == NULL)
        base_path = strdup(fn);
    if (strcmp(base_path, fn) != 0)
        fprintf(stderr, "*info* %s -> %s\n", fn, base_path);

    *devs = (deviceDetails *) realloc(*devs, (1024 * sizeof(deviceDetails))); // todo, max 1024 drives XXX
    (*devs)[*numDevs].fd = 0;
    (*devs)[*numDevs].bdSize = 0;
    (*devs)[*numDevs].shouldBeSize = 0;
    (*devs)[*numDevs].devicename = base_path;
    (*devs)[*numDevs].exclusive = 0;
    (*devs)[*numDevs].isBD = 0;
    (*devs)[*numDevs].ctxIndex = 0;
    (*devs)[*numDevs].numa = -1;

    //    if (verbose >= 2) {
    //      fprintf(stderr,"*info* adding -f '%s'\n", (*devs)[*numDevs].devicename);
    //    }

    deviceDetails *dcmp = &(*devs)[*numDevs];
    (*numDevs)++;
    return dcmp;
}

void freeDeviceDetails(deviceDetails *devs, size_t numDevs) {
    for (size_t f = 0; f < numDevs; f++) {
        if (devs[f].devicename) {
            free(devs[f].devicename);
        }
    }
    free(devs);
}

size_t loadDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs) {
    size_t add = 0;
    FILE *fp = fopen(fn, "rt");
    if (!fp) {
        perror(fn);
        return add;
    }

    char *line = NULL;
    size_t len = 0;
    ssize_t read;
    int numa = -1;
    int hasanuma = 0;

    while ((read = getline(&line, &len, fp)) != -1) {
        //    printf("Retrieved line of length %zu :\n", read);
        line[strlen(line) - 1] = 0; // erase the \n
        numa = -1;

        char *space = strchr(line, ' ');
        if (space) {
            *space = 0;
            numa = atoi(space + 1);
            if (!hasanuma) hasanuma = 1;
        }

        deviceDetails *d = addDeviceDetails(line, devs, numDevs);
        if (d) {
            d->numa = numa;
            //    fprintf(stderr,"--> %s %d\n", line, numa);
            //    addDeviceToAnalyse(line);
            add++;
            //    printf("%s", line);
        }
    }

    free(line);
    if (hasanuma) {
        fprintf(stderr, "*info* specified drive list has a NUMA column\n");
    }

    fclose(fp);
    fp = NULL;
    return add;
}


int deleteFile(const char *filename) {
    int fd = open(filename, O_RDWR | O_EXCL, S_IRUSR | S_IWUSR);
    if (fd) { // if open
        int isFile = (isBlockDevice(filename) == 2);
        close(fd);
        if (isFile) {
            //      fprintf(stderr,"*warning* deleting old file '%s'\n", filename);
            unlink(filename);
            return 0;
        }
    }
    return 1;
}


size_t numOpenDevices(deviceDetails *devs, size_t numDevs) {
    size_t count = 0;
    for (size_t i = 0; i < numDevs; i++) {
        if (devs[i].fd > 0) {
            count++;
        }
    }
    return count;
}


void openDevices(deviceDetails *devs, size_t numDevs, const size_t sendTrim, size_t *maxSizeInBytes, size_t LOWBLKSIZE,
                 size_t BLKSIZE, size_t alignment, int needToWrite, int dontUseExclusive, const size_t contextCount) {

    if (sendTrim) {}
    //  fprintf(stderr,"needToWrite %d\n", needToWrite);

    //  int error = 0;
    assert(contextCount > 0);

    assert(maxSizeInBytes);
    char newpath[4096];
    //  CALLOC(newpath, 4096, sizeof(char));

    for (size_t i = 0; i < numDevs; i++) {
        if (!keepRunning) exit(-1);
        memset(newpath, 0, 4096);
        // follow a symlink
        devs[i].fd = -1;
        char *ret = realpath(devs[i].devicename, newpath);
        if (!ret) { // if the file doesn't exists, set defaults
            if (*maxSizeInBytes == 0) {
                *maxSizeInBytes = (totalRAM() * 2);
                devs[i].shouldBeSize = *maxSizeInBytes;
                fprintf(stderr, "*info* defaulting to 2 x RAM = %.1lf GiB (override with -G option)\n",
                        TOGiB(*maxSizeInBytes));
            }
            if (devs[i].shouldBeSize == 0) {
                devs[i].shouldBeSize = *maxSizeInBytes;
            }
            // create not there
            int rv = createFile(devs[i].devicename, devs[i].shouldBeSize);
            if (rv != 0) {
                fprintf(stderr, "*error* couldn't create file '%s'\n", devs[i].devicename);
                exit(-1);
            }

        } else {
            int isFile = (isBlockDevice(devs[i].devicename) == 2);
            size_t fsz = fileSizeFromName(devs[i].devicename);
            if (isFile && (fsz != devs[i].shouldBeSize)) {
                fprintf(stderr, "*info* deleting '%s' as file size is %zd, should be %zd\n", devs[i].devicename, fsz,
                        devs[i].shouldBeSize);
                deleteFile(devs[i].devicename);
                // create not there
                int rv = createFile(devs[i].devicename, devs[i].shouldBeSize);
                if (rv != 0) {
                    fprintf(stderr, "*error* couldn't create file '%s'\n", devs[i].devicename);
                    exit(-1);
                }

            }
        }




        // try and create it, or skip if it's already there
        strcpy(newpath, devs[i].devicename);

        size_t phy, log;
        devs[i].isBD = isBlockDevice(newpath);
        if (devs[i].isBD == 1) {
            /* -- From a Google search
            The physical block size is the smallest size the device can
            handle atomically. The smalles block it can handle without needing
            to do a read-modify-write cycle. Applications should try to write
            in multiples of that (and they do) but are not required to.

            The logical block size on the other hand is the smallest size the
            device can handle at all. Applications must write in multiples of
            that.
            */

            char *suffix = getSuffix(newpath);
            char *sched = getScheduler(suffix);

            size_t max_io_bytes = 256 * 1024;

            getPhyLogSizes(suffix, &phy, &max_io_bytes, &log);
            //      if (verbose >= 2) {
            //	fprintf(stderr,"  *info* physical_block_size %zd, logical_block_size %zd\n", phy, log);
            //      }

            if (LOWBLKSIZE < log) {
                fprintf(stderr, "  *warning* the block size is lower than the device logical block size\n");
                LOWBLKSIZE = log;
                if (LOWBLKSIZE > BLKSIZE) BLKSIZE = LOWBLKSIZE;
            }

            // various warnings and informational comments
            if (LOWBLKSIZE < alignment) {
                LOWBLKSIZE = alignment;
                if (LOWBLKSIZE > BLKSIZE) BLKSIZE = LOWBLKSIZE;
                fprintf(stderr, "*warning* setting -k [%zd-%zd] because of the alignment of %zd bytes\n",
                        LOWBLKSIZE / 1024, BLKSIZE / 1024, alignment);
            }


            free(sched);
            sched = NULL;
            free(suffix);
            suffix = NULL;

            if (needToWrite) { // if any writes
                if (dontUseExclusive < 3) { // specify at least -XXX to turn off exclusive
                    devs[i].fd = open(newpath, O_RDWR | O_DIRECT | O_EXCL | O_TRUNC);
                } else {
                    fprintf(stderr, "  *warning* opening %s without O_EXCL\n", newpath);
                    devs[i].fd = open(newpath, O_RDWR | O_DIRECT | O_TRUNC);
                }
            } else { // only reads
                devs[i].fd = open(newpath, O_RDONLY | O_DIRECT); // if no writes, don't need write access
            }
            if (devs[i].fd < 0) {
                perror(newpath);
                goto cont;
            }

            devs[i].bdSize = blockDeviceSizeFromFD(devs[i].fd);
            // if too small then exit
            if (devs[i].bdSize <= 1) {
                fprintf(stderr, "*fatal* block device is too small/missing.\n");
                exit(-1);
            }

            //      fprintf(stderr,"nnn %s\n", newpath);
            //      if (verbose >= 2)
            //	fprintf(stderr,"*info* block device: '%s' size %zd bytes (%.2lf GiB)\n", newpath, devs[i].bdSize, TOGiB(devs[i].bdSize));
        } else if (devs[i].isBD == 2) { // file
            if (startsWith("/dev/", newpath)) {
                fprintf(stderr,
                        "*warning* high chance of critical problem, the path '%s' is a file (NOT a block device)\n",
                        newpath);
            }
            devs[i].fd = open(newpath, O_RDWR | O_EXCL); // if a file
            if (devs[i].fd < 0) {
                fprintf(stderr, "*info* opened file\n");
                devs[i].fd = open(newpath, O_RDWR | O_EXCL); // if a file
                if (devs[i].fd < 0) {
                    //	  fprintf(stderr,"maybe a file to create?\n");
                    perror(newpath);
                    goto cont;
                }
                fprintf(stderr, "*warning* couldn't open in O_DIRECT mode (filesystem constraints)\n");
            }
            devs[i].bdSize = fileSize(devs[i].fd);

            //      fprintf(stderr,"*info* file: '%s' size %zd bytes (%.2lf GiB)\n", newpath, devs[i].bdSize, TOGiB(devs[i].bdSize));
        } else {
            fprintf(stderr, "*info* %s is neither a file nor block device\n", newpath);
            goto cont;
        }

        cont:
        {}


        // setup aio
        if (i == 0) {
            devs[i].ctxIndex = i;
        } else {
            if (i < contextCount) {
                devs[i].ctxIndex = i;
            } else {
                devs[i].ctxIndex = i % contextCount;
            }
        }


        devs[i].exclusive = !dontUseExclusive;
    } // i

}

void infoDevices(const deviceDetails *devList, const size_t devCount) {
    fprintf(stderr, "*info* infoDevices %zd\n", devCount);
    for (size_t f = 0; f < devCount; f++) {

        fprintf(stderr, "*info* [%2d] ", devList[f].fd);
        switch (devList[f].isBD) {
            case 0:
                fprintf(stderr, "?");
                break;
            case 1:
                fprintf(stderr, "B");
                break;
            case 2:
                fprintf(stderr, "F");
                break;
            default:
                fprintf(stderr, "x");
                break;
        }
        fprintf(stderr, " %-25s %11zd (%.1lf GiB, limited %.1lf GiB), excl=%d\n", devList[f].devicename,
                devList[f].bdSize, TOGiB(devList[f].bdSize), TOGiB(devList[f].shouldBeSize), devList[f].exclusive);
    }
}


deviceDetails *prune(deviceDetails *devList, size_t *devCount, const size_t blockSize) {
    deviceDetails *dd = NULL;

    int count = 0;
    for (size_t f = 0; f < *devCount; f++) {
        if (devList[f].fd > 0 && devList[f].bdSize > 2 * blockSize) {
            count++;
        } else {
            fprintf(stderr, "*warningj* culling '%s' as the size %zd isn't over %zd\n", devList[f].devicename,
                    devList[f].bdSize, 2 * blockSize);
        }
    }
    CALLOC(dd, count, sizeof(deviceDetails));
    count = 0;
    for (size_t f = 0; f < *devCount; f++) {
        if (devList[f].fd > 0 && devList[f].bdSize > 2 * blockSize) {
            dd[count] = devList[f];
            dd[count].devicename = strdup(dd[count].devicename);
            count++;
        } else {
            //      fprintf(stderr,"*info* skipping '%s'\n", devList[f].devicename);
        }
    }

    *devCount = count;
    return dd;
}

size_t smallestBDSize(deviceDetails *devList, size_t devCount) {
    size_t min = 0, sz = 0;

    for (size_t f = 0; f < devCount; f++) {
        sz = fileSizeFromName(devList[f].devicename);
        if ((f == 0) || (sz < min)) {
            min = sz;
        }
    }
    fprintf(stderr, "*info* min size is %zd (%.3lf GiB)\n", min, TOGiB(min));
    return min;
}


size_t getIOPSestimate(const char *fn, const size_t blocksize, const int verbose) {

    int fd = open(fn, O_RDONLY);
    // default speed is 20GB/s
    size_t iop = 20L * 1024 * 1024 * 1024 / blocksize; // 20GB/s / blocksize
    if (fd > 0) {
        int rr = 0;
        unsigned int major, minor;
        majorAndMinor(fd, &major, &minor);
        close(fd);


        if (major == 252) {
            // nsulate/HPSc
            iop = 50L * 1024 * 1024 * 1024 / blocksize; // if 50GB/s / blocksize
            if (iop < 10) iop = 10;
        } else if (major == 1) {
            //      fprintf(stderr,"*info* RAM\n");
            iop = 40L * 1024L * 1024 * 1024L / blocksize; // if RAM them 40GB/s / blocksize;
            if (iop < 10) iop = 10;
            if (iop > 5000000) iop = 5000000;
        } else if (major == 9) {
            //      fprintf(stderr,"*mdadm*\n");
            iop = 20L * 1024L * 1024 * 1024L / blocksize; // assume mdadm can do 20GB/s / blocksize;
            if (iop < 10) iop = 10;
        } else {
            char *suf = getSuffix(fn);
            rr = getRotational(suf);
            if (suf) {
                free(suf);
            }
            if (rr) { // rot
                iop = 300L * 1024 * 1024 / blocksize;  // 300MB/s rotational
                if (iop < 10) iop = 10; // but at least 10 IOPS
            } else {// not rot
                iop = 12L * 1024L * 1024 * 1024 / 8L / blocksize; // 12Gb/sec for one device
                if (iop < 10) iop = 10;
            }
        }
        fprintf(stderr,
                "*info* '%s' has major:minor %u:%u (rotate=%d), estimate %zd sequential %.1lf KiB IOPS, %.0lf MiB/s\n",
                fn, major, minor, rr, iop, blocksize / 1024.0, iop * blocksize / 1024.0 / 1024);

    } else {
        //    perror(fn);
        iop = 1;
        fprintf(stderr, "*warning* '%s' no idea, 1 IOPS!\n", fn);
    }


    if (verbose) {
        fprintf(stderr, "*info* I/O estimate '%s' is %zd IOPS x %zd = %.1lf GiB/s (%.1lf GiB/s)\n", fn, iop, blocksize,
                TOGiB(iop * blocksize), TOGiB(iop * blocksize * 8));
    }
    return iop;
}


char *majorBDToString(int major) {
    FILE *fp = fopen("/proc/devices", "rt");
    if (!fp) {
        return strdup("");
    }

    char *line = NULL;
    size_t len = 0;
    ssize_t read = 0;
    int skip = 1;

    while ((read = getline(&line, &len, fp)) != -1) {
        if (strlen(line) > 1) {
            if (skip) continue;
        }
        skip = 0;

        if (((strchr(line, ':') == NULL) && (strlen(line) > 3))) {
            int mmm;
            char name[NAME_MAX];
            int s = sscanf(line, "%d %s", &mmm, name);
            if (s == 2) {
                if (mmm == major)
                    return strdup(name);
            }
        }
    }
    fclose(fp);
    return strdup("");
}
      

  
