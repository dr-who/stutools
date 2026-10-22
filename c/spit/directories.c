#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <linux/limits.h>
#include <sys/types.h>
#include <sys/stat.h>

#include "utils.h"
#include "directories.h"
#include "logSpeed.h"

int keepRunning = 1;

void makeFile(const char *filename, const char *buffer, const size_t size, const size_t chunk) {

    int fd = open(filename, O_RDWR | O_CREAT | O_TRUNC | O_DIRECT, 0600);
    if (fd < 0) {
        fprintf(stderr, "can't create file %s, size %zd\n", filename, size);
    }
    for (size_t i = 0; i < size; i += 16384) {
        if (write(fd, buffer + i, chunk) < (int) chunk) {
            //      fprintf(stderr,"short write\n");
        }
    }
    close(fd);
}


void readFile(const char *filename, char *buffer, const size_t size, const size_t chunk) {

    int fd = open(filename, O_RDONLY | O_DIRECT, 0600);
    if (fd < 0) {
        fprintf(stderr, "can't open file %s, size %zd\n", filename, size);
    }
    for (size_t i = 0; i < size; i += 16384) {
        if (read(fd, buffer + i, chunk) < (int) chunk) {
            //      fprintf(stderr,"short write\n");
        }
    }
    close(fd);
}


void makeDirectories(const char *prefix, size_t KiB, size_t count, size_t chunk, logSpeedType *ls) {
    const size_t size = KiB * 1000;

    struct stat st = {0};
    char s[PATH_MAX];
    sprintf(s, "%s/testdir", prefix);

    if (stat(s, &st) == -1) {
        mkdir(s, 0700);
    }

    char *buffer = aligned_alloc(size * sizeof(char), 4096);
    if (!buffer) {
        fprintf(stderr, "can't malloc\n");
        exit(1);
    }

    if (ls) logSpeedReset(ls);

    for (size_t i = 0; i < count; i++) {
        char ss[PATH_MAX];
        sprintf(ss, "%s/testdir/file-%05zd", prefix, i);
        //    fprintf(stderr,"%s\n", ss);
        makeFile(ss, buffer, size, chunk);
        if (ls) logSpeedAdd2(ls, TOMiB(size), 1);
    }

    free(buffer);
}


void readDirectories(const char *prefix, size_t KiB, size_t count, size_t chunk, logSpeedType *ls) {
    const size_t size = KiB * 1000;

    char s[PATH_MAX];
    sprintf(s, "%s/testdir", prefix);

    char *buffer = aligned_alloc(size * sizeof(char), 4096);
    if (!buffer) {
        fprintf(stderr, "can't malloc\n");
        exit(1);
    }

    if (ls) logSpeedReset(ls);

    for (size_t i = 0; i < count; i++) {
        char ss[PATH_MAX];
        sprintf(ss, "%s/testdir/file-%05zd", prefix, i);
        //    fprintf(stderr,"%s\n", ss);
        readFile(ss, buffer, size, chunk);
        if (ls) logSpeedAdd2(ls, TOMiB(size), 1);
    }

    free(buffer);
}


int main(int argc, char *argv[]) {
  (void)argc;
  (void)argv;

  const size_t KB = 360;
  fprintf(stderr, "Starting test: %.1lf MB\n", KB / 1000.0);
  const size_t count = 10000;
  const char *directory = "test";

  struct stat st = {0};
  if (stat(directory, &st) == -1) {
    fprintf(stderr, "*error* please create a directory called '%s'\n", directory);
    exit(-1);
  }

  for (size_t r = 0; r < 10; r++) {
    const double start = timeAsDouble();
    makeDirectories(directory, KB, count, 16384, NULL);
    const double elapsed = timeAsDouble() - start;
    fprintf(stderr, "create/write: %zd in %.3lf seconds, %.2lf files per second (%zd MB, %.0lf MB/s)\n", count,
	    elapsed, count / elapsed, count * KB / 1000, count * KB / 1000.0 / elapsed);
  }

  for (size_t r = 0; r < 10; r++) {
    const double start = timeAsDouble();
    readDirectories(directory, KB, count, 16384, NULL);
    const double elapsed = timeAsDouble() - start;
    fprintf(stderr, "read: %zd in %.3lf seconds, %.2lf files per second (%zd MB, %.0lf MB/s)\n", count, elapsed,
	    count / elapsed, count * KB / 1000, count * KB / 1000.0 / elapsed);
  }


  return 0;
}



