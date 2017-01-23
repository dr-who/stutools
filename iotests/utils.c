#include <stdlib.h>
#include <fcntl.h>
#include <sys/time.h>

#include "utils.h"


struct timeval gettm()
{
  struct timeval now;
  gettimeofday(&now, NULL);
  return now;
}

double timedouble() {
  struct timeval now = gettm();
  double tm = (now.tv_sec * 1000000 + now.tv_usec);
  return tm/1000000.0;
}
