#define _GNU_SOURCE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <math.h>

#include "devices.h"

		 

void addDeviceDetails(const char *fn, deviceDetails **devs, size_t *numDevs) {
  *devs = (deviceDetails*) realloc(*devs, (*numDevs+1) * sizeof(deviceDetails));
  (*devs)[*numDevs].devicename = (char*)strdup(fn);
  if (verbose >= 2) {
    fprintf(stderr,"*info* adding -f '%s'\n", (*devs)[*numDevs].devicename);
  }

  (*numDevs)++;
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
  
  while ((read = getline(&line, &len, fp)) != -1) {
    //    printf("Retrieved line of length %zu :\n", read);
    line[strlen(line)-1] = 0; // erase the \n
    addDeviceDetails(line, devs, numDevs);
    //    addDeviceToAnalyse(line);
    add++;
    //    printf("%s", line);
  }
  
  free(line);

  fclose(fp);
  return add;
}



