#include <stddef.h>
#include <assert.h>
#include <sys/time.h>

#include "utilstime.h"

double timeAsDouble(void) {
    struct timeval now;
    gettimeofday(&now, NULL);
    double tm = ((double) now.tv_sec * 1000000.0) + now.tv_usec;
    assert(tm > 0);
    return tm / 1000000.0;
}

