#define _DEFAULT_SOURCE
#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>

#include "utils.h"

#define DEFAULT_TIMEOUT 30

#define DEVICE_STRING "/sys/block/%s/device/timeout"

int keepRunning = 0;

void timeoutArray(int *fdArray, const int len, const int timeoutSec) {
  for (size_t i =0; i < len; i++) {
    fprintf(stderr,"%d ", fdArray[i]);
    char buf[100];
    int l = sprintf(buf, "%d", timeoutSec);

    int w = write(fdArray[i], buf, l);
    if (w < 0) {
      perror(buf);
      exit(-1);
    }
    lseek(fdArray[i], 0, SEEK_SET); // rewind
  }
  fprintf(stderr,"  -> %d\n", timeoutSec);
}

void swap(int *value1, int *value2) {
  int temp = *value1;
  *value1 = *value2;
  *value2 = temp;
}

void permute(int *array, int *data, const int start, const int end,
	     int index, int r,
	     const int timeoutSec, const int delaybeforeMS, const int delayafterMS) {
  //  fprintf(stderr,"%d %d\n", index, r);
  if (index == r){
    usleep(delaybeforeMS * 1000);
    timeoutArray(data, r, timeoutSec);
    usleep(delayafterMS * 1000);
    timeoutArray(data, r, DEFAULT_TIMEOUT); // reset back to 
    
    return;
  }
  for (int i = start; i <= end && end-i+1 >= r-index; i++) {
    data[index] = array[i];
    permute(array, data, i+1, end, index+1, r, timeoutSec, delaybeforeMS, delayafterMS);
  }
  return;
}


int *fdArray = NULL;
int drives = 0;


int addDeviceToAnalyse(const char *fn) {
  fdArray = realloc(fdArray, (drives + 1) * sizeof(int));
  fdArray[drives] = open(fn, O_RDONLY | O_EXCL);
  if (fdArray[drives] < 0) {
    perror(fn);
    return 1;
  }
  fprintf(stderr,"*info* can open O_EXCL '%s'\n", fn);
  close(fdArray[drives]);

  char *str = getSuffix(fn);
  char buf[1000];
  sprintf(buf, DEVICE_STRING, str);
  fdArray[drives] = open(buf, O_RDWR);
  if (fdArray[drives] < 0) {
    perror(buf);
    return 1;
  }
  fprintf(stderr,"*info* opened '%s' for O_RDWR (fd %d)\n", buf, fdArray[drives]);
  
  drives++;
  return 0;
}



size_t loadSpecifiedFiles(const char *fn) {
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
    addDeviceToAnalyse(line);
    add++;
    //    printf("%s", line);
  }
  
  free(line);

  fclose(fp);
  return add;
}



int parity = 1;
int initialPause = 15;
int timeoutSec = 0;
int delaybeforeMS = 100;
int delayafterMS = 1000;

void handle_args(int argc, char *argv[]) {
  int opt;
  
  while ((opt = getopt(argc, argv, "I:m:t:a:b:P:")) != -1) {
    switch (opt) {
    case 'P':
      initialPause = atoi(optarg);
      break;
    case 'I':
      loadSpecifiedFiles(optarg);
      break;
    case 'm':
      parity = atoi(optarg);
      break;
    case 't':
      timeoutSec = atoi(optarg);
      break;
    case 'b':
      delaybeforeMS = atoi(optarg);
      break;
    case 'a':
      delayafterMS = atoi(optarg);
      break;
    }
  }
}


void usage() {
  fprintf(stderr,"./deviceTimeout -I ok.txt -m parity -t timeout -a after_ms -b before_ms -P initialPause\n\n");
  fprintf(stderr,"Generates a permutation of up to m parity drives and sets the timeouts.\n\n");
  fprintf(stderr,"Example:\n");
  fprintf(stderr,"   ./deviceTimeout -I 4drives.txt -m 2\n");
  fprintf(stderr,"   drives 1 2 set to 0 sec timeout, pause then back to 30 sec\n");
  fprintf(stderr,"   drives 1 3 set to 0 timeout, pause then back to 30\n");
  fprintf(stderr,"   drives 1 4 set to 0 timeout, pause then back to 30\n");
  fprintf(stderr,"   drives 2 3 set to 0 timeout, pause then back to 30\n");
  fprintf(stderr,"   drives 2 4 set to 0 timeout, pause then back to 30\n");
  fprintf(stderr,"   drives 3 4 set to 0 timeout, pause then back to 30\n");
}

int main(int argc, char *argv[]) {

  int *data = NULL;
  
  handle_args(argc, argv);

  if (fdArray) {
    CALLOC(data, parity, sizeof(int)); 
    fprintf(stderr,"*info* drives %d, parity %d, timeout %d secs (default %d s) (before %d ms, after %d ms)\n", drives, parity, timeoutSec, DEFAULT_TIMEOUT, delaybeforeMS, delayafterMS);
    if (initialPause) {
      fprintf(stderr,"Waiting for %d seconds to start (time to start your array)...", initialPause);
      fflush(stderr);
      sleep(initialPause);
      fprintf(stderr,"\n");
    }
    permute(fdArray, data, 0, drives-1, 0, parity, timeoutSec, delaybeforeMS, delayafterMS);
  } else {
    usage();
  }

  return 0;
}
